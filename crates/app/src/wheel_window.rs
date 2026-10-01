//! Native window policy shared by the drag detector, tray and IPC commands.
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{Emitter, PhysicalPosition, WebviewWindow};

static GENERATION: AtomicU32 = AtomicU32::new(0);

pub fn generation() -> u32 {
    GENERATION.load(Ordering::SeqCst)
}

/// Apply before the first show: skipping the taskbar alone does not make the
/// wheel a tool window, so shell/dock utilities can still treat it as an app.
fn configure_native(window: &WebviewWindow, passthrough: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::Foundation::{GetLastError, SetLastError, BOOL};
        use windows_sys::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongW, SetWindowLongW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_LAYERED,
            WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
        };

        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
        SetLastError(0);
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if style == 0 && GetLastError() != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // Preserve WebView2 transparency, click-through and all other flags.
        let mut tool_style = (style | WS_EX_TOOLWINDOW as i32) & !(WS_EX_APPWINDOW as i32);
        if passthrough {
            tool_style |= (WS_EX_TRANSPARENT | WS_EX_LAYERED) as i32;
        } else {
            tool_style &= !(WS_EX_TRANSPARENT as i32);
        }
        if tool_style != style {
            SetLastError(0);
            if SetWindowLongW(hwnd, GWL_EXSTYLE, tool_style) == 0 && GetLastError() != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
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

        // Per-window only; never change system-wide animation preferences.
        // This disables DWM transitions; third-party custom effects may have
        // their own exclusion rules, independent of DWM.
        let disabled: BOOL = 1;
        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED as u32,
            (&disabled as *const BOOL).cast(),
            std::mem::size_of::<BOOL>() as u32,
        );
        if result < 0 {
            tracing::warn!(hresult = result, "Could not disable wheel DWM transitions");
        }
    }
    #[cfg(not(target_os = "windows"))]
    window
        .set_ignore_cursor_events(passthrough)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_passthrough(window: &WebviewWindow, passthrough: bool) -> Result<(), String> {
    // Tao rewrites GWL_EXSTYLE from its cached flags for this setter. On
    // Windows modify only the native bits so TOOLWINDOW survives every toggle.
    configure_native(window, passthrough)
}

/// Called on the UI thread. Drag summons must not activate the window, since
/// Explorer still owns the OLE drag session. Tray/IPC summons may take focus.
pub fn show(
    window: &WebviewWindow,
    position: Option<PhysicalPosition<i32>>,
    focus: bool,
) -> Result<(), String> {
    // Invalidate exit completions from an earlier appearance, even if the
    // frontend has not received the new show event yet.
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
    set_passthrough(window, false)?;

    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetForegroundWindow, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SWP_SHOWWINDOW,
        };
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
        let mut flags = SWP_NOACTIVATE | SWP_NOSIZE | SWP_SHOWWINDOW;
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
    set_passthrough(window, true)?;
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

pub fn toggle(window: &WebviewWindow) -> Result<(), String> {
    if window.is_visible().map_err(|e| e.to_string())? {
        window
            .emit("wheel_close_requested", ())
            .map_err(|e| e.to_string())
    } else {
        show(window, None, true)
    }
}
