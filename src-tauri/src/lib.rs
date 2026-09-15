pub mod cmdline;
mod commands;
pub mod engine;
pub mod import;
mod job;
pub mod launch;
pub mod machine;
pub mod manifest;
pub mod profile;
pub mod settings;
pub mod system;

use engine::Engine;
use settings::{AppSettings, ExitBehavior};
use std::sync::{Mutex, MutexGuard};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

pub struct AppState {
    settings: Mutex<AppSettings>,
    pub engine: Engine,
}

impl AppState {
    pub fn settings(&self) -> MutexGuard<'_, AppSettings> {
        self.settings.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { settings: Mutex::new(AppSettings::load()), engine: Engine::default() })
        .invoke_handler(tauri::generate_handler![
            commands::overview,
            commands::set_data_root,
            commands::save_machine,
            commands::set_exit_behavior,
            commands::list_profiles,
            commands::preview,
            commands::save_profile,
            commands::import_minis,
            commands::engine_start,
            commands::engine_stop,
            commands::engine_status,
            commands::engine_log,
            commands::app_exit,
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // X con il motore acceso riduce nella tray: il motore non si ferma chiudendo una finestra.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<AppState>().engine.is_running() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("avvio di Aethera non riuscito");
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn request_quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.engine.is_running() {
        app.exit(0);
        return;
    }
    let behavior = state.settings().exit_behavior;
    match behavior {
        ExitBehavior::Stop => {
            let _ = state.engine.stop();
            app.exit(0);
        }
        ExitBehavior::Leave => {
            state.engine.detach();
            app.exit(0);
        }
        ExitBehavior::Ask => {
            show_main(app);
            let _ = app.emit("aethera://ask-exit", ());
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Apri Aethera", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "Ferma il motore", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Esci…", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &stop, &separator, &quit])?;
    let mut tray = TrayIconBuilder::with_id("aethera")
        .tooltip("Aethera")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "stop" => {
                let _ = app.state::<AppState>().engine.stop();
            }
            "quit" => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
