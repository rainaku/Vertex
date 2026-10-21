//! Native window policy shared by the drag detector and IPC commands.
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{Emitter, PhysicalPosition, WebviewWindow};

static GENERATION: AtomicU32 = AtomicU32::new(0);

pub fn generation() -> u32 {
    GENERATION.load(Ordering::SeqCst)
}

/// Apply before the first show: skipping the taskbar alone does not make the
/// wheel a tool window, so shell/dock utilities can still treat it as an app.
fn configure_native(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    unsafe {
        configure_hwnd(window.hwnd().map_err(|e| e.to_string())?.0 as _)?;
    }
    #[cfg(not(target_os = "windows"))]
    window
        .set_ignore_cursor_events(false)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(windows)]
unsafe fn configure_hwnd(hwnd: windows_sys::Win32::Foundation::HWND) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_LAYERED,
        WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };

    SetLastError(0);
    let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
    if style == 0 && GetLastError() != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // WebView2 uses DWM transparency. Leaving WS_EX_LAYERED set makes
    // User32 hit-test a stale/zero-alpha bitmap instead of the WebView.
    let mut tool_style = (style | WS_EX_TOOLWINDOW as i32) & !(WS_EX_APPWINDOW as i32);
    tool_style &= !((WS_EX_TRANSPARENT | WS_EX_LAYERED) as i32);
    if tool_style != style {
        SetLastError(0);
        if SetWindowLongW(hwnd, GWL_EXSTYLE, tool_style) == 0 && GetLastError() != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // Flush User32's cached extended styles when recovering an old
        // layered surface. Otherwise hit testing can lag behind GWL_EXSTYLE.
        if SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
}

pub fn set_passthrough(window: &WebviewWindow, _passthrough: bool) -> Result<(), String> {
    // Tao rewrites GWL_EXSTYLE from its cached flags for this setter. On
    // Windows modify only the native bits so TOOLWINDOW survives every toggle.
    // Hidden HWNDs already pass input to the desktop. Never enable layered
    // hit testing on a visible surface, including through a delayed IPC call.
    configure_native(window)
}

/// Called on the UI thread. Drag summons must not activate the window, since
/// Explorer still owns the OLE drag session. Settings may take focus.
pub fn show(
    window: &WebviewWindow,
    position: Option<PhysicalPosition<i32>>,
    focus: bool,
) -> Result<(), String> {
    // Invalidate exit completions from an earlier appearance, even if the
    // frontend has not received the new show event yet.
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
    set_passthrough(window, false)?;
    if position.is_none() {
        window.center().map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetForegroundWindow, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SWP_SHOWWINDOW,
        };
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
        let mut flags = SWP_NOSIZE | SWP_SHOWWINDOW;
        if !focus {
            flags |= SWP_NOACTIVATE;
        }
        let (x, y) = match position {
            Some(pos) => (pos.x, pos.y),
            None => {
                flags |= SWP_NOMOVE;
                (0, 0)
            }
        };
        // Move and show atomically; no restore/minimize animation or first
        // visible frame at the previous drag's position.
        if SetWindowPos(hwnd, HWND_TOPMOST, x, y, 0, 0, flags) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if focus && SetForegroundWindow(hwnd) == 0 {
            tracing::debug!("Windows declined foreground activation for the wheel");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(position) = position {
            window.set_position(position).map_err(|e| e.to_string())?;
        }
        window.show().map_err(|e| e.to_string())?;
        if focus {
            window.set_focus().map_err(|e| e.to_string())?;
        }
    }
    window
        .emit(
            "wheel_shown",
            serde_json::json!({ "generation": generation, "focus": focus }),
        )
        .map_err(|e| e.to_string())
}

pub fn hide(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        };
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
        // Pair the native show with a native hide; Tao's cached VISIBLE flag
        // does not track SWP_SHOWWINDOW and can otherwise make hide a no-op.
        if SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_HIDEWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    #[cfg(not(target_os = "windows"))]
    window.hide().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn invalidate_close() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

pub fn initialize(window: &WebviewWindow) -> Result<(), String> {
    configure_native(window)?;
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
        };
        let disabled: i32 = 1;
        let result = DwmSetWindowAttribute(
            window.hwnd().map_err(|e| e.to_string())?.0 as _,
            DWMWA_TRANSITIONS_FORCEDISABLED as u32,
            (&disabled as *const i32).cast(),
            4,
        );
        if result < 0 {
            tracing::warn!(hresult = result, "Could not disable wheel DWM transitions");
        }
    }
    Ok(())
}

#[cfg(windows)]
pub fn position_for_drag(
    window: &WebviewWindow,
    point: windows_sys::Win32::Foundation::POINT,
    right: bool,
) -> Result<PhysicalPosition<i32>, String> {
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    };
    unsafe {
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let work = info.rcWork;
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
        // GetDpiForWindow is aware of this window's DPI context. Move the hidden
        // window to the target monitor first, allowing WM_DPICHANGED to run.
        if SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            point.x,
            point.y,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let dpi = GetDpiForWindow(hwnd).max(96) as i32;
        let width = (480 * dpi / 96).min(work.right - work.left);
        let height = (480 * dpi / 96).min(work.bottom - work.top);
        let margin = 24 * dpi / 96;
        let x = (if right {
            point.x - margin
        } else {
            point.x - width + margin
        })
        .clamp(work.left, (work.right - width).max(work.left));
        let y = (point.y - height / 2).clamp(work.top, (work.bottom - height).max(work.top));
        // Move to the destination DPI before showing; use physical target size.
        if SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(PhysicalPosition::new(x, y))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    };

    struct TestWindow(HWND);
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        DefWindowProcW(hwnd, msg, w, l)
    }

    #[test]
    fn visible_window_recaptures_native_hit_testing_after_passthrough() {
        unsafe {
            let class: Vec<u16> = "VertexInputRegression\0".encode_utf16().collect();
            let instance = GetModuleHandleW(std::ptr::null());
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(window_proc);
            wc.hInstance = instance;
            wc.lpszClassName = class.as_ptr();
            wc.hbrBackground = windows_sys::Win32::Graphics::Gdi::GetStockObject(
                windows_sys::Win32::Graphics::Gdi::BLACK_BRUSH,
            ) as _;
            assert_ne!(RegisterClassW(&wc), 0);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP,
                40,
                40,
                48,
                48,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let _window = TestWindow(hwnd);
            for cycle in 0..128 {
                // Recreate the old hidden-window state, including its stale
                // alpha surface, then call the production capture function.
                ShowWindow(hwnd, SW_HIDE);
                let previous_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
                SetWindowLongW(
                    hwnd,
                    GWL_EXSTYLE,
                    (previous_style | WS_EX_LAYERED | WS_EX_TRANSPARENT) as i32,
                );
                // A hidden transparent surface can retain a zero-alpha frame.
                // An interactive WebView must not keep layered-window hit testing.
                assert_ne!(SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA), 0);
                configure_hwnd(hwnd).unwrap();
                assert_ne!(
                    SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        40,
                        40,
                        48,
                        48,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW
                    ),
                    0
                );
                assert_eq!(
                    GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & (WS_EX_TRANSPARENT | WS_EX_LAYERED),
                    0,
                );
                let mut message = std::mem::zeroed();
                while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                windows_sys::Win32::Graphics::Dwm::DwmFlush();
                assert_eq!(
                    WindowFromPoint(POINT { x: 64, y: 64 }),
                    hwnd,
                    "visible window passes mouse input to the window behind it on cycle {cycle}"
                );
                let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
                assert_eq!(
                    style & (WS_EX_TRANSPARENT | WS_EX_LAYERED),
                    0,
                    "interactive window retains native click-through/layered flags"
                );
                assert_ne!(style & WS_EX_TOOLWINDOW, 0);
                assert_eq!(style & WS_EX_APPWINDOW, 0);
            }
        }
    }
}
