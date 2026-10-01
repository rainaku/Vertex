use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use vertex_core::{detect_format, Availability, CancelToken, Category, Format, Options, Registry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    pub format: Format,
    pub label: String,
    pub extension: String,
    pub category: Category,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetFormatInfo {
    pub format: Format,
    pub label: String,
    pub extension: String,
    pub category: Category,
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInfo {
    pub files: Vec<FormatInfo>,
    pub common_targets: Vec<TargetFormatInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressPayload {
    pub job_id: String,
    pub progress: f32, // 0.0 - 1.0
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertResult {
    pub success: bool,
    pub output_path: String,
    pub target_format: Format,
    pub error: Option<String>,
}

pub struct AppState {
    pub registry: Arc<Registry>,
    pub active_cancels: Arc<Mutex<std::collections::HashMap<String, CancelToken>>>,
}

pub struct LanguageMenu {
    pub toggle: tauri::menu::MenuItem<tauri::Wry>,
    pub settings: tauri::menu::MenuItem<tauri::Wry>,
    pub quit: tauri::menu::MenuItem<tauri::Wry>,
}

#[tauri::command]
pub fn set_app_language(language: String, menu: tauri::State<'_, LanguageMenu>) -> Result<(), String> {
    let (toggle, settings, quit) = match language.as_str() {
        "vi" => ("Hiện / ẩn Vertex", "Cài đặt nâng cao…", "Thoát Vertex"),
        "en" => ("Show / hide Vertex", "Advanced settings…", "Quit Vertex"),
        _ => return Err("Unsupported language".into()),
    };
    menu.toggle.set_text(toggle).map_err(|e| e.to_string())?;
    menu.settings.set_text(settings).map_err(|e| e.to_string())?;
    menu.quit.set_text(quit).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn detect_file(path: String) -> Result<FormatInfo, String> {
    let p = PathBuf::from(&path);
    let fmt = detect_format(&p).map_err(|e| e.to_string())?;
    let filename = p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(FormatInfo {
        format: fmt,
        label: fmt.label().to_string(),
        extension: fmt.extension().to_string(),
        category: fmt.category(),
        filename,
    })
}

#[tauri::command]
pub async fn get_available_targets(
    from: Format,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<TargetFormatInfo>, String> {
    let targets = state.registry.available_targets(from);
    let list = targets
        .into_iter()
        .map(|(fmt, avail)| {
            let (available, reason) = match avail {
                Availability::Ok => (true, None),
                Availability::Missing(r) => (false, Some(r)),
            };
            TargetFormatInfo {
                format: fmt,
                label: fmt.label().to_string(),
                extension: fmt.extension().to_string(),
                category: fmt.category(),
                available,
                reason,
            }
        })
        .collect();

    Ok(list)
}

#[tauri::command]
pub async fn get_batch_info(
    paths: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<BatchInfo, String> {
    let mut files = Vec::new();
    let mut formats = Vec::new();

    for p_str in &paths {
        let p = PathBuf::from(p_str);
        let fmt = detect_format(&p).map_err(|e| format!("{}: {}", p_str, e))?;
        let filename = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        files.push(FormatInfo {
            format: fmt,
            label: fmt.label().to_string(),
            extension: fmt.extension().to_string(),
            category: fmt.category(),
            filename,
        });
        formats.push(fmt);
    }

    let common = state.registry.batch_targets(&formats);
    let common_targets = common
        .into_iter()
        .map(|(fmt, avail)| {
            let (available, reason) = match avail {
                Availability::Ok => (true, None),
                Availability::Missing(r) => (false, Some(r)),
            };
            TargetFormatInfo {
                format: fmt,
                label: fmt.label().to_string(),
                extension: fmt.extension().to_string(),
                category: fmt.category(),
                available,
                reason,
            }
        })
        .collect();

    Ok(BatchInfo {
        files,
        common_targets,
    })
}

#[tauri::command]
pub async fn convert_file(
    app: AppHandle,
    job_id: String,
    input_path: String,
    target_format: Format,
    options: Option<Options>,
    state: tauri::State<'_, AppState>,
) -> Result<ConvertResult, String> {
    let registry = state.registry.clone();
    let opts = options.unwrap_or_default();
    let cancel = CancelToken::new();

    // Notify drag detector that conversion is in progress
    crate::drag_detector::set_is_converting(true);

    // Register cancel token
    {
        let mut map = state.active_cancels.lock().unwrap();
        map.insert(job_id.clone(), cancel.clone());
    }

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    let progress_cb = move |p: f32| {
        let _ = app_clone.emit(
            "convert_progress",
            ProgressPayload {
                job_id: job_id_clone.clone(),
                progress: p,
                status: if p >= 1.0 {
                    "done".to_string()
                } else {
                    "converting".to_string()
                },
            },
        );
    };

    let input = PathBuf::from(&input_path);
    let result = tokio::task::spawn_blocking(move || {
        registry.convert(&input, target_format, &opts, &progress_cb, &cancel)
    })
    .await
    .map_err(|e| e.to_string())?;

    // Cleanup cancel token
    {
        let mut map = state.active_cancels.lock().unwrap();
        map.remove(&job_id);
    }

    crate::drag_detector::set_is_converting(false);

    match result {
        Ok(out_path) => {
            let path_str = out_path.to_string_lossy().to_string();

            // Emit final done event
            let _ = app.emit(
                "convert_progress",
                ProgressPayload {
                    job_id: job_id.clone(),
                    progress: 1.0,
                    status: "done".to_string(),
                },
            );

            Ok(ConvertResult {
                success: true,
                output_path: path_str,
                target_format,
                error: None,
            })
        }
        Err(e) => {
            let err_msg = e.to_string();
            let _ = app.emit(
                "convert_progress",
                ProgressPayload {
                    job_id: job_id.clone(),
                    progress: 0.0,
                    status: "error".to_string(),
                },
            );

            Ok(ConvertResult {
                success: false,
                output_path: String::new(),
                target_format,
                error: Some(err_msg),
            })
        }
    }
}

#[tauri::command]
pub async fn cancel_job(job_id: String, state: tauri::State<'_, AppState>) -> Result<bool, String> {
    crate::drag_detector::set_is_converting(false);
    let mut map = state.active_cancels.lock().unwrap();
    if let Some(token) = map.remove(&job_id) {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    let safe_path = vertex_core::security::SafeLauncher::sanitize_explorer_path(p)?;

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", safe_path.display()))
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(&safe_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        let parent = safe_path.parent().unwrap_or(&safe_path);
        let _ = std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn hide_wheel_window(app: AppHandle, generation: u32) -> Result<(), String> {
    if generation != crate::wheel_window::generation() {
        return Ok(());
    }
    crate::drag_detector::set_is_converting(false);
    if let Some(win) = app.get_webview_window("main") {
        crate::wheel_window::hide(&win)?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_converting_state(converting: bool) {
    crate::drag_detector::set_is_converting(converting);
}

#[tauri::command]
pub fn show_wheel_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        crate::wheel_window::show(&win, None, true)?;
    }
    Ok(())
}

/// Toggle whether the window passes cursor events through to the OS
/// passthrough=true → invisible to mouse (click-through), passthrough=false → captures mouse
#[tauri::command]
pub fn set_window_passthrough(app: AppHandle, passthrough: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        crate::wheel_window::set_passthrough(&win, passthrough)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPos {
    pub x: i32,
    pub y: i32,
}

/// Get the current OS cursor position (screen coordinates)
#[tauri::command]
pub fn get_cursor_pos(app: AppHandle) -> Result<CursorPos, String> {
    // Use the monitor + cursor position from the window
    if let Some(win) = app.get_webview_window("main") {
        // Tauri 2: use the window's position and the webview's inner cursor
        // We return (0,0) as fallback; real pos comes from drag events
        let _ = win;
    }
    Ok(CursorPos { x: 0, y: 0 })
}
