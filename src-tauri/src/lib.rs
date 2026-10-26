pub mod commands;
pub mod db;
pub mod models;
pub mod services;

use tauri::Manager;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use services::task_queue::TaskQueue;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 尽早初始化日志系统（使用可执行文件目录）
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // 数据目录迁移刷新：必须在日志初始化之前 —— 补迁 set_data_dir 与重启之间
    // 产生的新数据（仅 DB/索引等 PDF 相关资料；日志/模型固定安装目录，不再迁移）。
    // 失败时内部回退引导设置到源目录，应用按旧数据完整启动（零丢失）。
    let effective_data_dir = commands::settings::finish_pending_data_dir_migration(&exe_dir);

    // 初始化基本日志到安装目录（写 <安装目录>/logs，与 get_settings 上报口径一致）
    init_early_logging(&exe_dir);

    info!("启动应用程序");
    info!("可执行文件目录: {:?}", exe_dir);
    info!("生效数据目录: {:?}", effective_data_dir);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            info!("开始初始化应用");

            // 初始化数据库
            debug!("初始化数据库...");
            let db = match db::init_database(app.handle()) {
                Ok(d) => {
                    info!("数据库初始化成功");
                    d
                }
                Err(e) => {
                    error!("数据库初始化失败: {}", e);
                    return Err(e);
                }
            };
            app.manage(std::sync::Mutex::new(db));

            // 获取数据目录
            debug!("获取配置的数据目录...");
            let data_dir = db::get_configured_data_dir(
                &app.state::<std::sync::Mutex<rusqlite::Connection>>().lock().unwrap(),
                app.handle()
            );

            // 初始化 PDF 服务
            debug!("初始化PDF服务...");
            let pdf_service = match services::pdf_service::PdfService::new() {
                Ok(s) => {
                    info!("PDF服务初始化成功");
                    s
                }
                Err(e) => {
                    error!("PDF服务初始化失败: {}", e);
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("PDF服务初始化失败: {}", e)
                    )));
                }
            };
            app.manage(std::sync::Mutex::new(pdf_service));

            // 日志/模型固定放在安装目录（可执行文件目录），不随数据目录迁移
            let install_dir = db::default_data_dir(app.handle());
            // 升级兼容：旧版本把模型放在数据目录里 → 一次性复制到安装目录（不覆盖已有）
            commands::settings::adopt_legacy_models(&install_dir, &data_dir);
            let models_dir = install_dir.join("models");

            // 初始化 OCR 服务（模型目录 = 安装目录/models）
            debug!("初始化OCR服务...");
            let ocr_service = match services::ocr_service::OcrService::new(&models_dir) {
                Ok(s) => {
                    info!("OCR服务初始化成功");
                    s
                }
                Err(e) => {
                    warn!("OCR服务初始化失败: {} (OCR功能将不可用)", e);
                    // OCR 服务失败不是致命错误，继续启动
                    services::ocr_service::OcrService::new(&models_dir)
                        .expect("Failed to initialize OCR service")
                }
            };
            app.manage(std::sync::Mutex::new(ocr_service));

            // 应用持久化的「启用模型」设置（ocr_active_model；缺省=Mobile，
            // 旧版本写下的 "balanced" 视为同一个模型：两者加载完全相同的文件）
            {
                let active = app
                    .state::<std::sync::Mutex<rusqlite::Connection>>()
                    .lock()
                    .ok()
                    .and_then(|conn| db::get_setting(&conn, db::SETTING_OCR_ACTIVE_MODEL));
                let parsed: Option<services::model_manager::ModelType> = match active.as_deref() {
                    None | Some("balanced") | Some("mobile") => {
                        Some(services::model_manager::ModelType::Mobile)
                    }
                    Some("disabled") => None,
                    Some(v) => match v.parse::<services::model_manager::ModelType>() {
                        Ok(t) => Some(t),
                        Err(e) => {
                            warn!("持久化模型设置无法解析: {}, {}", v, e);
                            Some(services::model_manager::ModelType::Mobile)
                        }
                    },
                };
                if let Ok(mut svc) = app.state::<std::sync::Mutex<services::ocr_service::OcrService>>().lock() {
                    svc.set_active_model(parsed);
                    info!("启动应用启用模型设置: {:?}", parsed);
                }
            }

            // 确保目录存在：PDF 相关随数据目录；日志/模型固定安装目录
            debug!("创建数据目录...");
            let dirs = [
                ("数据目录", &data_dir),
                ("PDF目录", &data_dir.join("pdfs")),
                ("缩略图目录", &data_dir.join("thumbnails")),
                ("索引目录", &data_dir.join("index")),
                ("日志目录", &install_dir.join("logs")),
                ("模型目录", &models_dir),
            ];

            for (name, path) in &dirs {
                match std::fs::create_dir_all(path) {
                    Ok(_) => debug!("{}创建成功: {:?}", name, path),
                    Err(e) => {
                        error!("{}创建失败: {:?}, 错误: {}", name, path, e);
                        return Err(Box::new(e));
                    }
                }
            }

            // 初始化搜索服务
            debug!("初始化搜索服务...");
            let index_path = data_dir.join("index");
            let mut search_service = match services::search_service::SearchService::open(&index_path) {
                Ok(s) => {
                    info!("搜索服务初始化成功");
                    s
                }
                Err(e) => {
                    error!("搜索服务初始化失败: {}", e);
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("搜索服务初始化失败: {}", e)
                    )));
                }
            };
            // T1 安全网：索引版本升级（如 v5→v6）会重建空索引；
            // 若不回填，老用户升级后搜索会静默变空（只能靠 force 重识别恢复）
            let refilled = {
                let db_state = app.state::<std::sync::Mutex<rusqlite::Connection>>();
                let n = match db_state.lock() {
                    Ok(conn) => match commands::search::refill_index_from_db(&conn, &mut search_service) {
                        Ok(n) => n,
                        Err(e) => {
                            warn!("索引回填失败: {}", e);
                            0
                        }
                    },
                    Err(e) => {
                        warn!("索引回填获取数据库锁失败: {}", e);
                        0
                    }
                };
                n
            };
            if refilled > 0 {
                info!("索引版本升级后从数据库回填: {} 页", refilled);
            }
            app.manage(std::sync::Mutex::new(search_service));

            // 初始化任务队列
            debug!("初始化任务队列...");
            let mut task_queue = TaskQueue::new();

            // T4：启动时恢复上次持久化的 pending OCR 任务
            let restored_ids: Vec<i64> = {
                let db_state = app.state::<std::sync::Mutex<rusqlite::Connection>>();
                let restored = match db_state.lock() {
                    Ok(conn) => commands::ocr::restore_persisted_queue(&conn, &mut task_queue),
                    Err(e) => {
                        warn!("恢复持久化 OCR 队列失败: {}", e);
                        Vec::new()
                    }
                };
                restored
            };
            app.manage(std::sync::Mutex::new(task_queue));

            if !restored_ids.is_empty() {
                info!("检测到持久化 OCR 队列，后台自动续跑: {:?}", restored_ids);
                let resume_handle = app.handle().clone();
                std::thread::spawn(move || {
                    commands::ocr::resume_ocr_queue(resume_handle);
                });
            }

            info!("应用初始化完成, 数据目录: {:?}, 安装目录: {:?}", data_dir, install_dir);
            info!("日志文件位置: {:?}", install_dir.join("logs"));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::folder::get_folders,
            commands::folder::create_folder,
            commands::folder::rename_folder,
            commands::folder::delete_folder,
            commands::pdf::add_pdf,
            commands::import::import_pdf_folder,
            commands::pdf::get_pdf_list,
            commands::pdf::delete_pdf,
            commands::pdf::move_pdfs,
            commands::pdf::get_pdf_detail,
            commands::pdf::render_pdf_page,
            commands::search::search,
            commands::search::search_filename,
            commands::search::get_page_matches,
            commands::search::get_page_ocr_info,
            commands::ocr::get_ocr_status,
            commands::ocr::get_ocr_download_guide,
            commands::ocr::get_model_overview,
            commands::ocr::set_active_model,
            commands::ocr::refresh_ocr_status,
            commands::ocr::download_ocr_models,
            commands::ocr::cancel_ocr_download,
            commands::ocr::start_ocr,
            commands::ocr::get_ocr_queue_status,
            commands::ocr::cancel_ocr_task,
            commands::ocr::get_memory_info,
            commands::settings::get_settings,
            commands::settings::set_data_dir,
            commands::settings::reset_data_dir,
            commands::settings::set_pdf_reader,
            commands::settings::set_ocr_max_image_dimension,
            commands::settings::set_ocr_preprocess_mode,
            commands::settings::open_pdf_externally,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 初始化早期日志（生成日志文件）
///
/// `base_dir` = 安装目录（可执行文件目录）：日志固定写 `<安装目录>/logs`，
/// 与 `get_settings.log_dir` 上报一致；选数据目录不会把日志搬走。
fn init_early_logging(base_dir: &std::path::Path) {
    use tracing_subscriber::fmt::time::LocalTime;
    use time::macros::format_description;
    use time::OffsetDateTime;

    let log_dir = base_dir.join("logs");

    // 创建日志目录
    let _ = std::fs::create_dir_all(&log_dir);

    // 日志文件名
    let today = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    let date_str = today.format(format_description!("[year]-[month]-[day]")).unwrap_or("unknown".to_string());
    let log_filename = format!("{}-pdf-ocr.log", date_str);
    let log_path = log_dir.join(&log_filename);

    // 尝试创建日志文件
    let file_appender = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(f) => f,
        Err(_) => {
            // 如果无法创建文件，只输出到控制台
            eprintln!("无法创建日志文件，仅输出到控制台");
            let _ = tracing_subscriber::fmt()
                .with_target(true)
                .with_line_number(true)
                .with_timer(LocalTime::new(format_description!("[year]-[month]-[day] [hour]:[minute]:[second]")))
                .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                .try_init();
            return;
        }
    };

    // 初始化日志系统，同时输出到文件和控制台
    let timer = LocalTime::new(format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"));

    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::sync::Mutex::new(file_appender))
                .with_ansi(false)
                .with_target(true)
                .with_line_number(true)
                .with_timer(timer.clone())
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .with_target(true)
                .with_line_number(true)
                .with_timer(timer)
        )
        .with(tracing_subscriber::EnvFilter::new("info"))
        .try_init();

    info!("日志系统初始化成功，日志文件: {:?}", log_path);
}