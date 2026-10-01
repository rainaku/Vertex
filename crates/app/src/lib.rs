pub mod commands;
pub mod drag_detector;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tracing::info;
use vertex_core::Registry;

use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
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
            show_wheel_window,
            set_window_passthrough,
            set_converting_state,
            get_cursor_pos,
        ])
        .setup(|app| {
            // Build Tray Menu
            let toggle_i = MenuItem::with_id(app, "toggle", "Hiện / Ẩn Vertex", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Thoát Vertex", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&toggle_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .tooltip("Vertex - File Converter Wheel")
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "toggle" => {
                            if let Some(win) = app.get_webview_window("main") {
                                if win.is_visible().unwrap_or(false) {
                                    let _ = win.hide();
                                } else {
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                }
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
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
                            if win.is_visible().unwrap_or(false) {
                                let _ = win.hide();
                            } else {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Main window setup: start hidden and in passthrough mode
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
                let _ = window.set_ignore_cursor_events(true);

                #[cfg(target_os = "macos")]
                {
                    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                    let _ = apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, None);
                }
            }

            // Start global Ctrl+Drag watcher thread
            drag_detector::start_drag_detector(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Vertex application");
}
