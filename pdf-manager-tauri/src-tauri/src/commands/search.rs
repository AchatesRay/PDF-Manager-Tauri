use crate::services::search_service::{SearchResult, SearchService};
use std::sync::Mutex;
use tauri::State;

/// 搜索
#[tauri::command]
pub fn search(
    query: String,
    folder_id: Option<i64>,
    search_service: State<'_, Mutex<SearchService>>,
) -> Result<Vec<SearchResult>, String> {
    let svc = search_service.lock().map_err(|e| e.to_string())?;
    svc.search(&query, folder_id, 50).map_err(|e| e.to_string())
}