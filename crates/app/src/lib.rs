pub mod commands;
pub mod drag_detector;
#[cfg(any(windows, test))]
mod drag_gesture;
mod logging;
mod tray_menu;
mod wheel_window;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tracing::info;
use vertex_core::Registry;

use commands::*;

struct SingleInstanceGuard {
    #[cfg(windows)]
    handle: isize,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.handle != 0 {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.handle as _);
            }
        }
    }
}

fn acquire_single_instance_guard() -> Option<SingleInstanceGuard> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::CreateMutexW;

        let name: Vec<u16> = "Local\\VertexSingleInstanceMutex\0"
            .encode_utf16()
            .collect();
        let handle = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
        if handle.is_null() {
            return None;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(handle);
            return None;
        }
        Some(SingleInstanceGuard {
            handle: handle as isize,
        })
    }
    #[cfg(not(windows))]
    {
        Some(SingleInstanceGuard {})
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let Some(_instance_guard) = acquire_single_instance_guard() else {
        return;
    };

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
            set_window_passthrough,
            set_converting_state,
            set_app_language,
            tray_menu::get_tray_state,
            tray_menu::tray_action,
            center_window,
            prepare_settings_window,
            restore_wheel_window,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                use tauri_plugin_autostart::ManagerExt;
                if let Ok(true) = app.autolaunch().is_enabled() {
                    let _ = app.autolaunch().disable();
                }
            }

            tray_menu::setup(app)?;

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

#[cfg(all(test, windows))]
mod tests {
    use super::acquire_single_instance_guard;

    #[test]
    fn single_instance_guard_blocks_second_instance_until_dropped() {
        let first = acquire_single_instance_guard();
        assert!(first.is_some(), "first instance should acquire mutex");
        let second = acquire_single_instance_guard();
        assert!(second.is_none(), "second instance must be blocked");
        drop(first);
        let third = acquire_single_instance_guard();
        assert!(third.is_some(), "instance after drop should acquire mutex");
    }
}
