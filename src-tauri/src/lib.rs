pub mod commands;
pub mod db;
pub mod models;
pub mod services;

use tauri::Manager;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    info!("启动应用程序");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
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

            // 初始化日志系统 (使用配置的数据目录)
            debug!("获取配置的数据目录...");
            let data_dir = db::get_configured_data_dir(
                &app.state::<std::sync::Mutex<rusqlite::Connection>>().lock().unwrap(),
                app.handle()
            );
            init_logging(&data_dir);

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

            // 初始化 OCR 服务
            debug!("初始化OCR服务...");
            let ocr_service = match services::ocr_service::OcrService::new(&data_dir) {
                Ok(s) => {
                    info!("OCR服务初始化成功");
                    s
                }
                Err(e) => {
                    warn!("OCR服务初始化失败: {} (OCR功能将不可用)", e);
                    // OCR 服务失败不是致命错误，继续启动
                    services::ocr_service::OcrService::new(&data_dir)
                        .expect("Failed to initialize OCR service")
                }
            };
            app.manage(std::sync::Mutex::new(ocr_service));

            // 确保数据目录存在
            debug!("创建数据目录...");
            let dirs = [
                ("数据目录", &data_dir),
                ("PDF目录", &data_dir.join("pdfs")),
                ("缩略图目录", &data_dir.join("thumbnails")),
                ("索引目录", &data_dir.join("index")),
                ("日志目录", &data_dir.join("logs")),
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
            let search_service = match services::search_service::SearchService::open(&index_path) {
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
            app.manage(std::sync::Mutex::new(search_service));

            info!("应用初始化完成, 数据目录: {:?}", data_dir);
            info!("日志文件位置: {:?}", data_dir.join("logs"));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::folder::get_folders,
            commands::folder::create_folder,
            commands::folder::rename_folder,
            commands::folder::delete_folder,
            commands::pdf::add_pdf,
            commands::pdf::get_pdf_list,
            commands::pdf::delete_pdf,
            commands::pdf::get_pdf_detail,
            commands::search::search,
            commands::search::search_filename,
            commands::ocr::get_ocr_status,
            commands::ocr::start_ocr,
            commands::settings::get_settings,
            commands::settings::set_data_dir,
            commands::settings::reset_data_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_logging(data_dir: &std::path::Path) {
    let log_dir = data_dir.join("logs");

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("创建日志目录失败: {:?}, 错误: {}", log_dir, e);
        return;
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "pdf-ocr.log");

    // 从环境变量获取日志级别，默认为info
    let log_level = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "info".to_string());

    match tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(false)
                .with_line_number(true)
        )
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level))
        )
        .try_init()
    {
        Ok(_) => {
            info!("日志系统初始化成功");
            info!("日志目录: {:?}", log_dir);
            info!("日志级别: {}", log_level);
        }
        Err(e) => {
            eprintln!("日志系统初始化失败: {}", e);
        }
    }
}