pub mod commands;
pub mod db;
pub mod models;
pub mod services;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 初始化数据库
            let db = db::init_database(app.handle())?;
            app.manage(std::sync::Mutex::new(db));

            // 初始化 PDF 服务
            let pdf_service = services::pdf_service::PdfService::new()
                .expect("Failed to initialize PDF service");
            app.manage(std::sync::Mutex::new(pdf_service));

            // 确保数据目录存在
            let data_dir = db::get_data_dir(app.handle());
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(db::get_pdfs_dir(app.handle()))?;
            std::fs::create_dir_all(db::get_thumbnails_dir(app.handle()))?;
            std::fs::create_dir_all(db::get_index_dir(app.handle()))?;

            tracing::info!("Application initialized successfully");

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
            commands::ocr::get_ocr_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}