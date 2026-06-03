mod commands;
mod errors;
mod models;
mod process;
mod storage;
mod svn;

use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};

// 应用菜单的自定义项发往前端的事件名，前端据此切换视图或触发刷新
const MENU_ACTION_EVENT: &str = "menu-action";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .on_menu_event(|app, event| {
            let id = event.id().0.as_str();
            // 标准编辑/窗口项由系统处理，这里只转发自定义的视图/操作项
            if id.starts_with("view:") || id.starts_with("action:") {
                let _ = app.emit(MENU_ACTION_EVENT, id.to_string());
            }
        })
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

            // 构建原生应用菜单，补齐 macOS 标准的 应用/编辑/视图/窗口 菜单
            let handle = app.handle();
            let app_menu = SubmenuBuilder::new(handle, "SVN Client")
                .about(None)
                .separator()
                .services()
                .separator()
                .hide()
                .hide_others()
                .separator()
                .quit()
                .build()?;
            let edit_menu = SubmenuBuilder::new(handle, "编辑")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;
            let view_menu = SubmenuBuilder::new(handle, "视图")
                .text("view:status", "状态")
                .text("view:log", "历史")
                .text("view:remote", "远端")
                .text("view:checkout", "检出")
                .separator()
                .text("action:refresh", "刷新")
                .build()?;
            let window_menu = SubmenuBuilder::new(handle, "窗口")
                .minimize()
                .fullscreen()
                .separator()
                .close_window()
                .build()?;
            let menu = MenuBuilder::new(handle)
                .item(&app_menu)
                .item(&edit_menu)
                .item(&view_menu)
                .item(&window_menu)
                .build()?;
            app.set_menu(menu)?;

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
            commands::set_working_copy_display_name,
            commands::list_working_copy_files,
            commands::create_working_copy_folder,
            commands::svn_get_info,
            commands::svn_get_status,
            commands::svn_get_status_stream,
            commands::svn_get_log,
            commands::svn_get_diff,
            commands::svn_get_diff_revision,
            commands::svn_get_base_content,
            commands::read_file_text,
            commands::reveal_in_file_manager,
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
