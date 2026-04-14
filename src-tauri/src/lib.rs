mod accumulator;
mod commands;
mod csv;
mod http;
mod queries;
mod query;
mod types;
mod utils;

#[cynic::schema("startgg")]
mod schema {}

pub use commands::{
    get_tournament_json, get_tournament_rows_csv, get_tournament_rows_json, greet, save_text_file,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    commands::create_tauri_app()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
