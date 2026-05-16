// src-tauri/src/main.rs
// Application entry point. Sets up Tauri v2 app with plugins and state.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use metadata_cleaner_lib::commands::AppState;
use metadata_cleaner_lib::types::AppSettings;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            settings: Mutex::new(AppSettings::default()),
            db_path: PathBuf::from(
                dirs::data_local_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("metadata-cleaner")
                    .join("history.db"),
            ),
            cancelled: Mutex::new(false),
        })
        .setup(|_app| {
            let data_dir = dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("metadata-cleaner");

            if !data_dir.exists() {
                let _ = std::fs::create_dir_all(&data_dir);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            metadata_cleaner_lib::commands::scan_files_cmd,
            metadata_cleaner_lib::commands::scan_single_file,
            metadata_cleaner_lib::commands::clean_files_cmd,
            metadata_cleaner_lib::commands::get_history_cmd,
            metadata_cleaner_lib::commands::restore_file_cmd,
            metadata_cleaner_lib::commands::detect_tools,
            metadata_cleaner_lib::commands::get_settings,
            metadata_cleaner_lib::commands::update_settings,
            metadata_cleaner_lib::commands::cancel_operation,
            metadata_cleaner_lib::commands::reset_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
