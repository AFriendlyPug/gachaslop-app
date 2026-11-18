//! GachaSlop tray app: game reset countdowns and banners in a small popup above the Windows tray.
//! Each module only uses the ones listed above it.
mod time;
mod config;
mod logs;
mod net;
mod games;
mod banners;
mod update;
mod settings;
mod store;
mod autostart;
mod instance;
mod tray;
mod widget;
mod toast;
mod scheduler;
mod commands;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use tauri::{Manager, RunEvent};

use crate::instance::Start;
use crate::settings::Paths;
use crate::store::Store;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // one copy at a time: a second launch shows the running copy's popup instead
    let listener = match instance::claim() {
        Start::First(listener) => Some(listener),
        Start::AlreadyRunning => return,
        Start::Unguarded => None,
    };

    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::save_settings,
            commands::refresh_banners,
            commands::mark_notices_read,
            commands::clear_notices,
            commands::check_for_update,
            commands::install_update,
            commands::dismiss_update,
            commands::set_dialog_open,
            commands::stage_game_icon,
            commands::discard_staged,
            commands::add_game,
            commands::update_game,
            commands::delete_game,
            commands::preview_reset,
            commands::test_notification,
            commands::replay_catchup,
            commands::set_tray_only,
            commands::diagnostics,
            commands::open_path,
            commands::hide_popup,
            commands::quit,
        ])
        .setup(move |app| {
            logs::init(&app.path().app_log_dir()?);
            let paths = Paths { root: app.path().app_data_dir()? };
            std::fs::create_dir_all(&paths.root)?;
            let store = Arc::new(Store::open(paths));
            commands::clear_staged(&store.paths); // icons picked for a game that was never saved
            let _ = std::fs::remove_dir_all(store.paths.updates()); // an installer already run, or abandoned
            {
                // the registry is the truth for "Start with Windows" (it can be turned off in Task Manager)
                let mut inner = store.lock();
                inner.settings.autostart = autostart::is_enabled();
            }
            app.manage(store.clone());
            tray::build(app.handle(), &store)?;
            widget::start(app.handle()); // on the taskbar by the clock; the tray icon only if it can't be
            match listener {
                Some(listener) => {
                    let handle = app.handle().clone();
                    instance::serve(listener, move || {
                        let main = handle.clone();
                        let _ = handle.run_on_main_thread(move || tray::show_popup(&main));
                    });
                }
                None => logs::warn("Port in use by another program: a second copy of the app could start"),
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || scheduler::run(handle, store));
            logs::info("Started");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the app");

    app.run(|_app, event| match event {
        // keep running (the popup only hides) unless Quit asked to exit, which passes an exit code
        RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
        RunEvent::Exit => widget::stop(),
        _ => {}
    });
}
