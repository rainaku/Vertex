pub mod commands;
pub mod drag_detector;
#[cfg(any(windows, test))]
mod drag_gesture;
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
        conversion_slots: Arc::new(tokio::sync::Semaphore::new(
            std::thread::available_parallelism()
                .map(|n| (n.get() / 4).clamp(1, 2))
                .unwrap_or(1),
        )),
        active_cancels: Arc::new(Mutex::new(HashMap::new())),
    };

    tauri::Builder::default()
        .manage(state)
        .on_webview_event(|webview, event| {
            #[cfg(windows)]
            if webview.label() == "main" {
                let tauri::WebviewEvent::DragDrop(event) = event else { return; };
                drag_detector::on_native_drag_event(webview.app_handle(), event);
            }
            #[cfg(not(windows))]
            let _ = (webview, event);
        })
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            detect_file,
            get_input_formats,
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
                MenuItem::with_id(app, "toggle", "Hiện / ẩn Vertex", true, None::<&str>)?;
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
                        if let Some(win) = app.get_webview_window("main") {
                            if let Err(error) =
                                win.emit("open_advanced_settings", ())
                                        .map_err(|e| e.to_string())
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

            // Hidden native windows already let mouse input reach the desktop.
            if let Some(window) = app.get_webview_window("main") {
                wheel_window::initialize(&window).map_err(std::io::Error::other)?;
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
            let integrity_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                if vertex_core::security::AppIntegrityService::is_dev_or_test_environment() {
                    return;
                }

                if let Ok(exe_path) = std::env::current_exe() {
                    let (is_trusted, origin_url) =
                        vertex_core::security::AppIntegrityService::check_download_origin(
                            &exe_path,
                        );
                    if !is_trusted {
                        if let Some(url) = origin_url {
                            tracing::warn!(
                                %url,
                                "Application was downloaded from an untrusted origin"
                            );
                            let _ = integrity_handle.emit("untrusted_origin", url);
                        } else {
                            tracing::warn!("Application was downloaded from an untrusted origin");
                        }
                    }
                }
            });

            // Start background auto-updater check
            let update_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Wait 15 seconds after app startup
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                loop {
                    use tauri_plugin_updater::UpdaterExt;
                    match update_handle.updater() {
                        Ok(updater) => {
                            match updater.check().await {
                                Ok(Some(update)) => {
                                    let version = update.version.clone();
                                    let notes = update.body.clone().unwrap_or_default();
                                    tracing::info!(%version, "New Vertex version available");

                                    // Trigger desktop notification
                                    use tauri_plugin_notification::NotificationExt;
                                    let body = match current_language() {
                                        "en" => format!(
                                            "Update v{version} downloaded. Restart Vertex to apply."
                                        ),
                                        _ => format!(
                                            "Đã tải bản cập nhật v{version}. Khởi động lại ứng dụng để áp dụng."
                                        ),
                                    };
                                    let _ = update_handle
                                        .notification()
                                        .builder()
                                        .title("Vertex")
                                        .body(body)
                                        .show();

                                    // Notify frontend
                                    let _ = update_handle.emit("update_available", serde_json::json!({
                                        "version": version,
                                        "notes": notes,
                                    }));
                                }
                                Ok(None) => {
                                    tracing::debug!("Vertex is running latest version");
                                }
                                Err(err) => {
                                    tracing::debug!(%err, "Background update check skipped/failed");
                                }
                            }
                        }
                        Err(err) => {
                            tracing::debug!(%err, "Updater extension not ready");
                        }
                    }
                    // Recheck every 6 hours
                    tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Vertex application");
}
