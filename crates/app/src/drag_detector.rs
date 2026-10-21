use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tauri::{AppHandle, Emitter, Manager};

static UI_BUSY: AtomicBool = AtomicBool::new(false);
static ACTIVE_JOBS: AtomicUsize = AtomicUsize::new(0);
static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static DRAG_OVER_WEBVIEW: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static DROP_SESSION: std::sync::Mutex<crate::drag_gesture::DropSession> =
    std::sync::Mutex::new(crate::drag_gesture::DropSession::new());

#[cfg(windows)]
pub fn on_native_drag_event(app: &AppHandle, event: &tauri::DragDropEvent) {
    if matches!(event, tauri::DragDropEvent::Over { .. })
        && DRAG_OVER_WEBVIEW.load(Ordering::Relaxed)
    {
        return;
    }
    let close = {
        let mut session = DROP_SESSION.lock().unwrap_or_else(|p| p.into_inner());
        match event {
            tauri::DragDropEvent::Enter { .. } | tauri::DragDropEvent::Over { .. } => {
                session.enter();
                DRAG_OVER_WEBVIEW.store(true, Ordering::Relaxed);
                false
            }
            tauri::DragDropEvent::Drop { .. } => {
                DRAG_OVER_WEBVIEW.store(false, Ordering::Relaxed);
                session.finish();
                false
            }
            tauri::DragDropEvent::Leave => {
                DRAG_OVER_WEBVIEW.store(false, Ordering::Relaxed);
                session.leave()
            }
            _ => false,
        }
    };
    if close {
        let _ = app.emit("shift_drag_end", ());
    }
}

pub fn set_is_converting(value: bool) {
    UI_BUSY.store(value, Ordering::Relaxed);
}
pub fn get_is_converting() -> bool {
    UI_BUSY.load(Ordering::Relaxed) || ACTIVE_JOBS.load(Ordering::Relaxed) > 0
}
pub fn begin_job() {
    ACTIVE_JOBS.fetch_add(1, Ordering::Relaxed);
}
pub fn finish_job() {
    ACTIVE_JOBS.fetch_sub(1, Ordering::Relaxed);
}
pub fn set_settings_open(value: bool) {
    SETTINGS_OPEN.store(value, Ordering::Relaxed);
}

pub fn settings_open() -> bool {
    SETTINGS_OPEN.load(Ordering::Relaxed)
}

#[cfg(windows)]
mod windows_drag {
    use super::*;
    use std::sync::atomic::AtomicIsize;
    use std::sync::{mpsc, OnceLock};
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_LSHIFT, VK_RBUTTON, VK_RSHIFT, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    #[derive(Clone, Copy)]
    enum Event {
        Down(POINT),
        Move(POINT),
        Up,
        Escape,
    }
    static EVENTS: OnceLock<mpsc::Sender<Event>> = OnceLock::new();
    static TRACK_MOVES: AtomicBool = AtomicBool::new(false);
    static LEFT_DOWN: AtomicBool = AtomicBool::new(false);
    static SHIFT_DOWN: AtomicBool = AtomicBool::new(false);
    static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);

    unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let message = wparam as u32;
            if message == WM_MOUSEMOVE {
                if !TRACK_MOVES.load(Ordering::Relaxed) {
                    return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
                }
                let point = (*(lparam as *const MSLLHOOKSTRUCT)).pt;
                if let Some(sender) = EVENTS.get() {
                    let _ = sender.send(Event::Move(point));
                }
                return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
            }
            if !matches!(
                message,
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONUP
            ) {
                return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
            }
            let primary_right = GetSystemMetrics(SM_SWAPBUTTON) != 0;
            let down = if primary_right {
                WM_RBUTTONDOWN
            } else {
                WM_LBUTTONDOWN
            };
            let up = if primary_right {
                WM_RBUTTONUP
            } else {
                WM_LBUTTONUP
            };
            let event = if message == down {
                if (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0 {
                    LEFT_DOWN.store(true, Ordering::Relaxed);
                    TRACK_MOVES.store(true, Ordering::Relaxed);
                    let point = (*(lparam as *const MSLLHOOKSTRUCT)).pt;
                    Some(Event::Down(point))
                } else {
                    None
                }
            } else if message == up {
                TRACK_MOVES.store(false, Ordering::Relaxed);
                if LEFT_DOWN.swap(false, Ordering::Relaxed) {
                    Some(Event::Up)
                } else {
                    None
                }
            } else {
                None
            };
            if let (Some(sender), Some(event)) = (EVENTS.get(), event) {
                let _ = sender.send(event);
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let key = (*(lparam as *const KBDLLHOOKSTRUCT)).vkCode;
            let is_shift = [VK_SHIFT, VK_LSHIFT, VK_RSHIFT]
                .iter()
                .any(|v| key == u32::from(*v));
            match wparam as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    if key == u32::from(VK_ESCAPE) {
                        TRACK_MOVES.store(false, Ordering::Relaxed);
                        if LEFT_DOWN.load(Ordering::Relaxed) {
                            if let Some(sender) = EVENTS.get() {
                                let _ = sender.send(Event::Escape);
                            }
                        }
                    } else if is_shift && !SHIFT_DOWN.swap(true, Ordering::Relaxed) {
                        let primary = if GetSystemMetrics(SM_SWAPBUTTON) != 0 {
                            VK_RBUTTON
                        } else {
                            VK_LBUTTON
                        };
                        let mut point = POINT { x: 0, y: 0 };
                        if GetAsyncKeyState(primary as i32) as u16 & 0x8000 != 0
                            && GetCursorPos(&mut point) != 0
                        {
                            LEFT_DOWN.store(true, Ordering::Relaxed);
                            TRACK_MOVES.store(true, Ordering::Relaxed);
                            if let Some(sender) = EVENTS.get() {
                                let _ = sender.send(Event::Down(point));
                            }
                        }
                    }
                }
                WM_KEYUP | WM_SYSKEYUP if is_shift => {
                    SHIFT_DOWN.store(false, Ordering::Relaxed);
                }
                _ => {}
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }

    fn blocked(app: &AppHandle) -> bool {
        if get_is_converting() || SETTINGS_OPEN.load(Ordering::Relaxed) {
            return true;
        }
        unsafe {
            let foreground = GetForegroundWindow();
            if foreground.is_null() || IsIconic(foreground) != 0 {
                return false;
            }
            let mut main_hwnd = MAIN_HWND.load(Ordering::Relaxed);
            if main_hwnd == 0 {
                if let Some(h) = app.get_webview_window("main").and_then(|w| w.hwnd().ok()) {
                    main_hwnd = h.0 as isize;
                    MAIN_HWND.store(main_hwnd, Ordering::Relaxed);
                }
            }
            if main_hwnd != 0 && foreground as isize == main_hwnd {
                return true;
            }
            if GetWindowLongW(foreground, GWL_STYLE) as u32 & WS_CAPTION != 0 {
                return false;
            }
            let monitor = windows_sys::Win32::Graphics::Gdi::MonitorFromWindow(
                foreground,
                windows_sys::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
            );
            let mut info: windows_sys::Win32::Graphics::Gdi::MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of_val(&info) as u32;
            let mut rect = std::mem::zeroed();
            if windows_sys::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut info) == 0
                || GetWindowRect(foreground, &mut rect) == 0
            {
                return false;
            }
            if rect.left > info.rcMonitor.left
                || rect.top > info.rcMonitor.top
                || rect.right < info.rcMonitor.right
                || rect.bottom < info.rcMonitor.bottom
            {
                return false;
            }
            // Borderless/exclusive fullscreen. Exempt the Explorer desktop.
            const PROGMAN: &[u16] = &[
                b'P' as u16,
                b'r' as u16,
                b'o' as u16,
                b'g' as u16,
                b'm' as u16,
                b'a' as u16,
                b'n' as u16,
            ];
            const WORKERW: &[u16] = &[
                b'W' as u16,
                b'o' as u16,
                b'r' as u16,
                b'k' as u16,
                b'e' as u16,
                b'r' as u16,
                b'W' as u16,
            ];
            let mut class = [0; 64];
            let len = GetClassNameW(foreground, class.as_mut_ptr(), class.len() as i32);
            let class = &class[..len.max(0) as usize];
            !matches!(class, PROGMAN | WORKERW)
        }
    }

    pub fn start(app: AppHandle) {
        let (sender, receiver) = mpsc::channel();
        if EVENTS.set(sender).is_err() {
            return;
        }
        std::thread::spawn(move || {
            let mut gesture = crate::drag_gesture::DragGesture::default();
            for event in receiver {
                use crate::drag_gesture::Action;
                let action = match event {
                    Event::Down(point) => {
                        gesture.press(point.x, point.y, !blocked(&app));
                        Action::None
                    }
                    Event::Move(point) => gesture.motion(
                        point.x,
                        point.y,
                        unsafe { GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000 } != 0
                            && !blocked(&app),
                    ),
                    Event::Up => gesture.release(),
                    Event::Escape => gesture.escape(),
                };
                TRACK_MOVES.store(
                    gesture.needs_motion() && LEFT_DOWN.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
                match action {
                    Action::Show { right } => {
                        let Event::Move(point) = event else {
                            continue;
                        };
                        let ui_app = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            if blocked(&ui_app) {
                                return;
                            }
                            if let Some(win) = ui_app.get_webview_window("main") {
                                DRAG_OVER_WEBVIEW.store(false, Ordering::Relaxed);
                                DROP_SESSION
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner())
                                    .begin();
                                match crate::wheel_window::position_for_drag(&win, point, right) {
                                    Ok(position) => {
                                        if crate::wheel_window::show(&win, Some(position), false)
                                            .is_ok()
                                        {
                                            let _ = ui_app.emit("shift_drag_start", ());
                                        }
                                    }
                                    Err(error) => {
                                        tracing::warn!(%error, "Could not position drag wheel")
                                    }
                                }
                            }
                        });
                    }
                    Action::End | Action::Cancel => {
                        let ui_app = app.clone();
                        let name = if action == Action::Cancel {
                            "drag_cancelled"
                        } else {
                            "shift_drag_end"
                        };
                        let _ = app.run_on_main_thread(move || {
                            let close = {
                                let mut session =
                                    DROP_SESSION.lock().unwrap_or_else(|p| p.into_inner());
                                if action == Action::Cancel {
                                    DRAG_OVER_WEBVIEW.store(false, Ordering::Relaxed);
                                    session.finish();
                                    true
                                } else {
                                    session.release()
                                }
                            };
                            if !close {
                                return;
                            }
                            let _ = ui_app.emit(name, ());
                        });
                    }
                    Action::None => {}
                }
            }
        });
        std::thread::spawn(|| unsafe {
            use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
            let module = GetModuleHandleW(std::ptr::null());
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), module, 0);
            let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), module, 0);
            if mouse.is_null() || keyboard.is_null() {
                if !mouse.is_null() {
                    UnhookWindowsHookEx(mouse);
                }
                if !keyboard.is_null() {
                    UnhookWindowsHookEx(keyboard);
                }
                tracing::error!(
                    "Could not install Shift+Drag hooks: {}",
                    std::io::Error::last_os_error()
                );
                return;
            }
            let mut message = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            UnhookWindowsHookEx(mouse);
            UnhookWindowsHookEx(keyboard);
        });
    }
}

pub fn start_drag_detector(app: AppHandle) {
    #[cfg(windows)]
    windows_drag::start(app);
    #[cfg(not(windows))]
    let _ = app;
}
