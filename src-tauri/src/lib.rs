pub mod adopt;
pub mod builds;
pub mod catalog;
pub mod clients;
pub mod cmdline;
pub mod conditions;
mod commands;
pub mod diagnose;
pub mod download;
pub mod endpoint;
pub mod engine;
pub mod estimate;
pub mod gguf;
pub mod hash;
pub mod import;
mod job;
pub mod launch;
pub mod machine;
pub mod manifest;
pub mod memory;
pub mod modelcard;
pub mod profile;
pub mod proposals;
pub mod provenance;
pub mod runs;
pub mod settings;
pub mod system;
pub mod tasks;
pub mod telemetry;
pub mod tray;

use engine::Engine;
use settings::{AppSettings, ExitBehavior};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;

pub struct AppState {
    settings: Mutex<AppSettings>,
    pub engine: Engine,
    /// Indirizzo dell'endpoint di Aethera, o perché non è partito.
    pub endpoint: Result<String, String>,
    /// Lavori lunghi del catalogo (hash, download, installazioni) con il loro avanzamento.
    pub tasks: tasks::Tasks,
    /// Serializza i cicli leggi-modifica-scrivi su `catalog.toml`.
    pub catalog_lock: Mutex<()>,
}

impl AppState {
    pub fn settings(&self) -> MutexGuard<'_, AppSettings> {
        self.settings.lock().unwrap_or_else(|e| e.into_inner())
    }
}

struct TrayItems {
    state: MenuItem<Wry>,
    model: MenuItem<Wry>,
    info: MenuItem<Wry>,
    protect: CheckMenuItem<Wry>,
    restart: MenuItem<Wry>,
    stop: MenuItem<Wry>,
    copy: MenuItem<Wry>,
}

pub fn run() {
    let engine = Engine::default();
    let endpoint = endpoint::spawn(engine.clone(), endpoint::DEFAULT_ADDR).map(|a| a.to_string());
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState {
            settings: Mutex::new(AppSettings::load()),
            engine,
            endpoint,
            tasks: tasks::Tasks::default(),
            catalog_lock: Mutex::new(()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::overview,
            commands::set_data_root,
            commands::save_machine,
            commands::set_exit_behavior,
            commands::machine_text,
            commands::machine_reset,
            commands::setup,
            commands::list_profiles,
            commands::preview,
            commands::save_profile,
            commands::import_minis,
            commands::profile_template,
            commands::profile_duplicate,
            commands::profile_rename,
            commands::profile_deletion_plan,
            commands::profile_delete,
            commands::catalog_sampling,
            commands::engine_start,
            commands::engine_stop,
            commands::engine_restart,
            commands::engine_protect,
            commands::engine_status,
            commands::engine_log,
            commands::engine_failure,
            commands::orphan_terminate,
            commands::client_snippets,
            commands::runs_list,
            commands::run_detail,
            commands::runs_compare,
            commands::catalog_list,
            commands::catalog_verify,
            commands::catalog_add,
            commands::catalog_download,
            commands::catalog_reread,
            commands::catalog_removal_plan,
            commands::catalog_remove,
            commands::catalog_relink,
            commands::catalog_import_plan,
            commands::catalog_import,
            commands::catalog_modelcard,
            commands::builds_import_dir,
            commands::builds_list,
            commands::builds_install,
            commands::build_devices,
            commands::tasks_list,
            commands::task_cancel,
            commands::tasks_clear,
            commands::app_exit,
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            spawn_watchers(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // X con il motore acceso riduce nella tray: il motore non si ferma chiudendo una finestra.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if state.engine.is_running() || !state.tasks.running().is_empty() {
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

fn notice(app: &AppHandle, text: impl Into<String>) {
    let _ = app.emit("aethera://notice", text.into());
}

fn request_quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    // Un lavoro in corso si chiede sempre, qualunque sia la preferenza sull'uscita: la preferenza
    // dice che cosa fare del motore, non che cosa fare di un download a metà.
    let busy = !state.tasks.running().is_empty();
    if !state.engine.is_running() && !busy {
        app.exit(0);
        return;
    }
    if busy {
        show_main(app);
        let _ = app.emit("aethera://ask-exit", ());
        return;
    }
    let behavior = state.settings().exit_behavior;
    match behavior {
        // Un motore in uso non si ferma in silenzio: si chiede.
        ExitBehavior::Stop if !state.engine.usage().in_use => {
            let _ = state.engine.stop();
            app.exit(0);
        }
        ExitBehavior::Leave => {
            state.engine.detach();
            app.exit(0);
        }
        _ => {
            show_main(app);
            let _ = app.emit("aethera://ask-exit", ());
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let items = TrayItems {
        state: MenuItem::with_id(app, "state", "SPENTO", false, None::<&str>)?,
        model: MenuItem::with_id(app, "model", "nessun motore acceso", false, None::<&str>)?,
        info: MenuItem::with_id(app, "info", " ", false, None::<&str>)?,
        protect: CheckMenuItem::with_id(app, "protect", "Proteggi il motore", true, false, None::<&str>)?,
        restart: MenuItem::with_id(app, "restart", "Riavvia con la stessa riga", false, None::<&str>)?,
        stop: MenuItem::with_id(app, "stop", "Ferma il motore", false, None::<&str>)?,
        copy: MenuItem::with_id(app, "copy", "Copia riga per i client", false, None::<&str>)?,
    };
    let open = MenuItem::with_id(app, "open", "Apri Aethera", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Esci…", true, None::<&str>)?;
    let (s1, s2) = (PredefinedMenuItem::separator(app)?, PredefinedMenuItem::separator(app)?);
    let menu = Menu::with_items(
        app,
        &[&items.state, &items.model, &items.info, &s1, &items.protect, &items.restart, &items.stop, &items.copy, &s2, &open, &quit],
    )?;
    let mut tray = TrayIconBuilder::with_id("aethera")
        .tooltip("Aethera")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let state = app.state::<AppState>();
            match event.id().as_ref() {
                "open" => show_main(app),
                "protect" => {
                    let on = app.state::<TrayItems>().protect.is_checked().unwrap_or(false);
                    state.engine.set_protected(on);
                }
                "stop" => {
                    if let Err(e) = state.engine.stop() {
                        notice(app, format!("Ferma dalla tray: {e}"));
                    }
                }
                "restart" => {
                    if let Err(e) = commands::restart(&state) {
                        notice(app, format!("Riavvia dalla tray: {e}"));
                    }
                }
                "copy" => {
                    if let Some(s) = commands::snippets_for(&state) {
                        let _ = app.clipboard().write_text(s.toml);
                    }
                }
                "quit" => request_quit(app),
                _ => {}
            }
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
    app.manage(items);
    Ok(())
}

/// Tray aggiornata ogni 2 s; orfani cercati ogni 3 s sulle porte dei profili, solo a motore spento.
fn spawn_watchers(app: AppHandle) {
    let tray_app = app.clone();
    std::thread::spawn(move || loop {
        let state = tray_app.state::<AppState>();
        let status = state.engine.status();
        let v = tray::view(&status, state.engine.usage().protected);
        let items = tray_app.state::<TrayItems>();
        let _ = items.state.set_text(&v.state);
        let _ = items.model.set_text(&v.model);
        let _ = items.info.set_text(if v.info.is_empty() { " " } else { &v.info });
        let _ = items.protect.set_checked(v.protected);
        let _ = items.restart.set_enabled(v.can_restart);
        let _ = items.stop.set_enabled(v.can_stop);
        let _ = items.copy.set_enabled(v.has_run);
        if let Some(t) = tray_app.tray_by_id("aethera") {
            let _ = t.set_tooltip(Some(&v.tooltip));
        }
        std::thread::sleep(Duration::from_secs(2));
    });

    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(3));
        let state = app.state::<AppState>();
        if state.engine.is_running() {
            continue;
        }
        let endpoints = commands::profile_endpoints(&state);
        state.engine.scan_orphans(&endpoints);
    });
}
