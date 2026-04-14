use crate::csv::rows_to_csv;
use crate::query::{query_tournament_json, query_tournament_rows};
use tauri::Emitter;

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub fn save_text_file(path: String, contents: String) -> Result<String, String> {
    match std::fs::write(&path, contents) {
        Ok(_) => Ok(format!("Saved file to {}", path)),
        Err(e) => Err(format!("Failed to save file to {}: {}", path, e)),
    }
}

#[tauri::command]
pub async fn get_tournament_json(tournament_name: String, auth_token: String) -> String {
    println!("Received tournament name: {}", tournament_name);
    query_tournament_json(tournament_name, auth_token).await
}

#[tauri::command]
pub async fn get_tournament_rows_json(
    app: tauri::AppHandle,
    tournament_names: Vec<String>,
    auth_token: String,
) -> Result<String, String> {
    match query_tournament_rows(tournament_names, auth_token, |progress| {
        let _ = app.emit("download-progress", progress);
    })
    .await
    {
        Ok(rows) => serde_json::to_string_pretty(&rows)
            .map_err(|e| format!("Failed to serialize rows as JSON: {}", e)),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn get_tournament_rows_csv(
    app: tauri::AppHandle,
    tournament_names: Vec<String>,
    auth_token: String,
) -> Result<String, String> {
    match query_tournament_rows(tournament_names, auth_token, |progress| {
        let _ = app.emit("download-progress", progress);
    })
    .await
    {
        Ok(rows) => rows_to_csv(&rows).map_err(|e| format!("CSV export failed: {}", e)),
        Err(e) => Err(e),
    }
}

pub fn create_tauri_app() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            save_text_file,
            get_tournament_json,
            get_tournament_rows_json,
            get_tournament_rows_csv
        ])
}
