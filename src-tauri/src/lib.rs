mod commands;
mod errors;
mod models;
mod process;
mod storage;
mod svn;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let state = storage::init_config_state(&app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::svn_check_environment,
            commands::get_svn_bin,
            commands::set_svn_bin,
            commands::list_repositories,
            commands::save_repository,
            commands::remove_repository,
            commands::test_repository_connection,
            commands::svn_list_remote,
            commands::svn_cat_remote,
            commands::list_working_copies,
            commands::add_working_copy,
            commands::remove_working_copy,
            commands::refresh_working_copy,
            commands::list_working_copy_files,
            commands::create_working_copy_folder,
            commands::svn_get_info,
            commands::svn_get_status,
            commands::svn_get_log,
            commands::svn_get_diff,
            commands::svn_get_diff_revision,
            commands::svn_get_base_content,
            commands::read_file_text,
            commands::svn_revert,
            commands::svn_add,
            commands::svn_delete,
            commands::svn_ignore,
            commands::svn_start_commit,
            commands::svn_start_update,
            commands::svn_start_checkout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
