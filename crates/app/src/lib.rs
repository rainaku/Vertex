pub mod commands;
pub mod drag_detector;
mod logging;
mod wheel_window;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tracing::info;
use vertex_core::Registry;

use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(logging::RedactedWriter::default)
        .init();
    info!("Initializing Vertex application");

    let state = AppState {
        registry: Arc::new(Registry::default()),
        active_cancels: Arc::new(Mutex::new(HashMap::new())),
    };

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            detect_file,
            get_available_targets,
            get_batch_info,
            convert_file,
            cancel_job,
            reveal_in_explorer,
            hide_wheel_window,
            force_hide_wheel_window,
            show_wheel_window,
            set_window_passthrough,
            set_converting_state,
            set_app_language,
            center_window,
            prepare_settings_window,
            restore_wheel_window,
        ])
        .setup(|app| {
            // Build Tray Menu
            let toggle_i =
                MenuItem::with_id(app, "toggle", "Hiện / Ẩn Vertex", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Thoát Vertex", true, None::<&str>)?;
            let settings_i =
                MenuItem::with_id(app, "settings", "Cài đặt nâng cao…", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&toggle_i, &settings_i, &quit_i])?;
            app.manage(LanguageMenu {
                toggle: toggle_i,
                settings: settings_i,
                quit: quit_i,
            });

            let _tray = TrayIconBuilder::with_id("vertex")
                .icon(app.default_window_icon().ok_or_else(|| std::io::Error::other("Missing Vertex app icon"))?.clone())
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .tooltip("Vertex")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "toggle" => {
                        if let Some(win) = app.get_webview_window("main") {
                            if let Err(error) = wheel_window::toggle(&win) {
                                tracing::warn!(%error, "Could not toggle wheel from tray menu");
                            }
                        }
                    }
                    "settings" => {
                        let _ = crate::commands::prepare_settings_window(app.clone());
                        if let Some(win) = app.get_webview_window("main") {
                            if let Err(error) =
                                wheel_window::show(&win, None, true).and_then(|_| {
                                    win.emit("open_advanced_settings", ())
                                        .map_err(|e| e.to_string())
                                 })
                            {
                                tracing::warn!(%error, "Could not open advanced settings");
                            }
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            if let Err(error) = wheel_window::toggle(&win) {
                                tracing::warn!(%error, "Could not toggle wheel from tray icon");
                            }
                        }
                    }
                })
                .build(app)?;

            // Main window setup: start hidden and in passthrough mode
            if let Some(window) = app.get_webview_window("main") {
                wheel_window::hide(&window).map_err(std::io::Error::other)?;

                #[cfg(target_os = "macos")]
                {
                    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                    let _ = apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, None);
                }
            }

            // Start global Ctrl+Drag watcher thread
            drag_detector::start_drag_detector(app.handle().clone());

            // Start background integrity check (Mark of the Web origin check)
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                if vertex_core::security::AppIntegrityService::is_dev_or_test_environment() {
                    return;
                }

                if let Ok(exe_path) = std::env::current_exe() {
                    let (is_trusted, _) =
                        vertex_core::security::AppIntegrityService::check_download_origin(
                            &exe_path,
                        );
                    if !is_trusted {
                        tracing::warn!("Application was downloaded from an untrusted origin");
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Vertex application");
}
