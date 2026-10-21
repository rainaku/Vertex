use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};

const LABEL: &str = "tray-menu";
const WIDTH: f64 = 248.0;
const HEIGHT: f64 = 132.0;

#[derive(Clone, serde::Serialize)]
pub struct TrayState {
    language: &'static str,
    version: String,
    converting: bool,
}

#[tauri::command]
pub fn get_tray_state(app: AppHandle) -> TrayState {
    TrayState {
        language: crate::commands::current_language(),
        version: app.package_info().version.to_string(),
        converting: crate::drag_detector::get_is_converting(),
    }
}

#[tauri::command]
pub fn tray_action(app: AppHandle, action: String) -> Result<(), String> {
    match action.as_str() {
        "settings" => {
            if crate::drag_detector::get_is_converting() {
                return Err("conversion_in_progress".into());
            }
            let main = app
                .get_webview_window("main")
                .ok_or("Main window unavailable")?;
            if let Some(menu) = app.get_webview_window(LABEL) {
                menu.hide().map_err(|e| e.to_string())?;
            }
            if crate::drag_detector::settings_open() {
                main.set_focus().map_err(|e| e.to_string())?;
            } else {
                main.emit("open_advanced_settings", ())
                    .map_err(|e| e.to_string())?;
            }
        }
        "quit" => app.exit(0),
        _ => return Err("Unknown tray action".into()),
    }
    Ok(())
}

pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    let menu =
        WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?view=tray".into()))
            .title("Vertex")
            .inner_size(WIDTH, HEIGHT)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .visible(false)
            .focused(false)
            .disable_drag_drop_handler()
            .build()?;
    let handle = app.handle().clone();
    menu.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Focused(false)) {
            if let Some(menu) = handle.get_webview_window(LABEL) {
                let _ = menu.hide();
            }
        }
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if let Some(menu) = handle.get_webview_window(LABEL) {
                let _ = menu.hide();
            }
        }
    });

    TrayIconBuilder::with_id("vertex")
        .icon(
            app.default_window_icon()
                .ok_or_else(|| std::io::Error::other("Missing Vertex app icon"))?
                .clone(),
        )
        .tooltip("Vertex")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left | MouseButton::Right,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                if let Err(error) = show(tray.app_handle(), position) {
                    tracing::warn!(%error, "Could not open tray menu");
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn show(app: &AppHandle, point: PhysicalPosition<f64>) -> tauri::Result<()> {
    let Some(menu) = app.get_webview_window(LABEL) else {
        return Ok(());
    };
    if let Some(monitor) = app.monitor_from_point(point.x, point.y)? {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let width = (WIDTH * scale).round() as u32;
        let height = (HEIGHT * scale).round() as u32;
        let gap = (8.0 * scale).round() as i32;
        let left = area.position.x + gap;
        let top = area.position.y + gap;
        let right = (area.position.x + area.size.width as i32 - width as i32 - gap).max(left);
        let bottom = (area.position.y + area.size.height as i32 - height as i32 - gap).max(top);
        let x = (point.x as i32 - width as i32 / 2).clamp(left, right);
        let y = (point.y as i32 - height as i32 - gap).clamp(top, bottom);
        menu.set_position(PhysicalPosition::new(x, y))?;
        menu.set_size(PhysicalSize::new(width, height))?;
    }
    menu.emit("tray_opened", get_tray_state(app.clone()))?;
    menu.show()?;
    menu.set_focus()?;
    Ok(())
}
