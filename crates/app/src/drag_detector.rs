use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};
use tracing::info;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::POINT;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_SHIFT,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// Global flag indicating whether a conversion is currently running,
/// so the background watcher doesn't hide the window while converting.
pub static IS_CONVERTING: AtomicBool = AtomicBool::new(false);

pub fn set_is_converting(val: bool) {
    IS_CONVERTING.store(val, Ordering::SeqCst);
}

pub fn get_is_converting() -> bool {
    IS_CONVERTING.load(Ordering::SeqCst)
}

/// Starts the global drag & drop detector thread.
pub fn start_drag_detector(app: AppHandle) {
    #[cfg(target_os = "windows")]
    {
        thread::spawn(move || {
            info!("Windows global Shift+Drag detector thread started");

            #[derive(Debug, PartialEq)]
            enum DragState {
                Idle,
                PotentialDrag { start_x: i32, start_y: i32 },
                Dragging,
            }

            let mut state = DragState::Idle;

            loop {
                thread::sleep(Duration::from_millis(15));

                // 1. Check ESC key to cancel immediately
                let esc_pressed = unsafe { (GetAsyncKeyState(VK_ESCAPE as i32) as u16 & 0x8000) != 0 };
                if esc_pressed {
                    if state != DragState::Idle {
                        state = DragState::Idle;
                        let ui_app = app.clone();
                        if let Err(error) = app.run_on_main_thread(move || {
                            let _ = ui_app.emit("drag_cancelled", ());
                            // The frontend completes its exit animation before
                            // asking Rust to hide the native surface.
                        }) {
                            tracing::warn!(%error, "Could not schedule wheel cancellation");
                        }
                    }
                    continue;
                }

                // 2. Check Shift key and Left Mouse Button
                let shift_down = unsafe { (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0 };
                let lbutton_down = unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0 };

                let mut pt = POINT { x: 0, y: 0 };
                unsafe { GetCursorPos(&mut pt) };

                match state {
                    DragState::Idle => {
                        // User started holding Shift + Left Button
                        if shift_down && lbutton_down {
                            state = DragState::PotentialDrag {
                                start_x: pt.x,
                                start_y: pt.y,
                            };
                        }
                    }
                    DragState::PotentialDrag { start_x, start_y } => {
                        if !shift_down || !lbutton_down {
                            // Released before moving enough (regular Shift+Click, do not trigger)
                            state = DragState::Idle;
                        } else {
                            let dx = (pt.x - start_x) as f32;
                            let dy = (pt.y - start_y) as f32;
                            let dist = (dx * dx + dy * dy).sqrt();

                            // Drag threshold: 8 pixels of movement
                            if dist >= 8.0 {
                                state = DragState::Dragging;
                                info!("Shift+Drag detected at ({}, {})! Positioning Vertex overlay...", pt.x, pt.y);

                                if let Some(win) = app.get_webview_window("main") {
                                    let size = win.outer_size().unwrap_or(tauri::PhysicalSize::new(480, 480));
                                    let w = size.width as i32;
                                    let h = size.height as i32;

                                    // Position menu on the side the user is dragging towards:
                                    // If dx >= 0 (dragged right): menu appears on the RIGHT of the cursor
                                    // If dx < 0 (dragged left): menu appears on the LEFT of the cursor
                                    // Move wheel further to the side so the dragged file thumbnail doesn't overlap
                                    // Cursor is placed 24px inside the window edge for instant OLE DragEnter
                                    let cursor_margin = 24;
                                    let mut target_x = if dx >= 0.0 {
                                        pt.x - cursor_margin
                                    } else {
                                        pt.x - (w - cursor_margin)
                                    };
                                    let mut target_y = pt.y - (h / 2);

                                    // Clamp to screen bounds so it never clips off display
                                    unsafe {
                                        use windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics;
                                        let min_x = GetSystemMetrics(76); // SM_XVIRTUALSCREEN
                                        let max_x = min_x + GetSystemMetrics(78) - w; // SM_CXVIRTUALSCREEN
                                        let min_y = GetSystemMetrics(77); // SM_YVIRTUALSCREEN
                                        let max_y = min_y + GetSystemMetrics(79) - h; // SM_CYVIRTUALSCREEN
                                        if max_x > min_x {
                                            target_x = target_x.clamp(min_x, max_x);
                                        }
                                        if max_y > min_y {
                                            target_y = target_y.clamp(min_y, max_y);
                                        }
                                    }

                                    // Keep native style/visibility changes on the window's
                                    // UI thread, before notifying the frontend of the drag.
                                    let ui_window = win.clone();
                                    let ui_app = app.clone();
                                    if let Err(error) = win.run_on_main_thread(move || {
                                        match crate::wheel_window::show(
                                            &ui_window,
                                            Some(PhysicalPosition::new(target_x, target_y)),
                                            false,
                                        ) {
                                            Ok(()) => {
                                                let _ = ui_app.emit("shift_drag_start", ());
                                                let _ = ui_app.emit("ctrl_drag_start", ());
                                            }
                                            Err(error) => {
                                                tracing::warn!(%error, "Could not show drag wheel");
                                            }
                                        }
                                    }) {
                                        tracing::warn!(%error, "Could not schedule drag wheel");
                                    }
                                }
                            }
                        }
                    }
                    DragState::Dragging => {
                        // The wheel stays open as long as the user is still dragging (Left Button down)
                        // Shift is only used as the trigger/summon key.
                        if !lbutton_down {
                            state = DragState::Idle;
                            info!("Left mouse button released, ending drag");
                            // Use the same queue as show/cancel so a quick release
                            // cannot reach the frontend before the start event.
                            let ui_app = app.clone();
                            if let Err(error) = app.run_on_main_thread(move || {
                                let _ = ui_app.emit("shift_drag_end", ());
                                let _ = ui_app.emit("ctrl_drag_end", ());
                            }) {
                                tracing::warn!(%error, "Could not schedule drag end");
                            }
                        }
                    }
                }
            }
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        info!("Non-Windows OS: relying on native webview drag & drop events");
    }
}
