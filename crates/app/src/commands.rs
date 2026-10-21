use serde::{Deserialize, Serialize};
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
    pub notices: Vec<vertex_core::contract::ConversionNotice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInfo {
    pub files: Vec<FormatInfo>,
    pub common_targets: Vec<TargetFormatInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressPayload<'a> {
    pub job_id: &'a str,
    pub progress: f32, // 0.0 - 1.0
    pub status: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertResult {
    pub cancelled: bool,
    pub success: bool,
    pub output_path: String,
    pub target_format: Format,
    pub error: Option<String>,
}

pub struct AppState {
    pub registry: Arc<Registry>,
    pub conversion_slots: Arc<tokio::sync::Semaphore>,
    pub active_cancels: Arc<Mutex<std::collections::HashMap<String, CancelToken>>>,
}

static APP_LANGUAGE: Mutex<&'static str> = Mutex::new("vi");

pub fn current_language() -> &'static str {
    *APP_LANGUAGE.lock().unwrap_or_else(|p| p.into_inner())
}

#[tauri::command]
pub fn set_app_language(language: String) -> Result<(), String> {
    let lang = match language.as_str() {
        "vi" => "vi",
        "en" => "en",
        _ => return Err("Unsupported language".into()),
    };
    *APP_LANGUAGE.lock().unwrap_or_else(|p| p.into_inner()) = lang;
    Ok(())
}

#[tauri::command]
pub fn get_input_formats(
    state: tauri::State<'_, AppState>,
) -> Vec<vertex_core::contract::FormatDefinition> {
    state.registry.source_formats()
}

#[tauri::command]
pub async fn detect_file(path: String) -> Result<FormatInfo, String> {
    tokio::task::spawn_blocking(move || {
        let p = vertex_core::path_utils::ipc_path(&path)?;
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
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_available_targets(
    from: Format,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<TargetFormatInfo>, String> {
    let registry = state.registry.clone();
    let targets = tokio::task::spawn_blocking(move || registry.available_targets(from))
        .await
        .map_err(|e| e.to_string())?;
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
                notices: vertex_core::contract::conversion_notice(from, fmt)
                    .into_iter()
                    .collect(),
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
    let registry = state.registry.clone();
    tokio::task::spawn_blocking(move || {
        let mut files = Vec::with_capacity(paths.len());
        let mut formats = Vec::with_capacity(paths.len());

        for p_str in &paths {
            let p = vertex_core::path_utils::ipc_path(p_str)?;
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

        let common = registry.batch_targets(&formats);
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
                    notices: formats
                        .iter()
                        .filter_map(|from| vertex_core::contract::conversion_notice(*from, fmt))
                        .fold(Vec::new(), |mut notices, notice| {
                            if !notices.contains(&notice) {
                                notices.push(notice);
                            }
                            notices
                        }),
                }
            })
            .collect();

        Ok(BatchInfo {
            files,
            common_targets,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

struct JobGuard {
    id: String,
    token: CancelToken,
    cancels: Arc<Mutex<std::collections::HashMap<String, CancelToken>>>,
}
impl JobGuard {
    fn register(
        id: &str,
        token: &CancelToken,
        cancels: &Arc<Mutex<std::collections::HashMap<String, CancelToken>>>,
    ) -> Result<Self, String> {
        let mut map = cancels.lock().unwrap_or_else(|p| p.into_inner());
        if map.contains_key(id) {
            return Err("Duplicate conversion job ID".into());
        }
        map.insert(id.to_owned(), token.clone());
        crate::drag_detector::begin_job();
        Ok(Self {
            id: id.to_owned(),
            token: token.clone(),
            cancels: cancels.clone(),
        })
    }
}
impl Drop for JobGuard {
    fn drop(&mut self) {
        self.token.cancel();
        self.cancels
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.id);
        crate::drag_detector::finish_job();
    }
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
    let input = vertex_core::path_utils::ipc_path(&input_path)?;
    let registry = state.registry.clone();
    let mut opts = options.unwrap_or_default().normalized();
    opts.output_dir = opts
        .output_dir
        .as_ref()
        .map(|path| {
            let raw = path
                .to_str()
                .ok_or_else(|| "Output directory must be valid Unicode".to_string())?;
            vertex_core::path_utils::ipc_path(raw)
        })
        .transpose()?;
    let cancel = CancelToken::new();
    let guard = JobGuard::register(&job_id, &cancel, &state.active_cancels)?;
    // Register before queuing, so queued jobs can be cancelled as well.
    let acquire = state.conversion_slots.clone().acquire_owned();
    tokio::pin!(acquire);
    let permit = loop {
        tokio::select! {
            permit = &mut acquire => break permit.map_err(|e| e.to_string())?,
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                if cancel.is_cancelled() {
                    let _ = app.emit(
                        "convert_progress",
                        ProgressPayload {
                            job_id: &job_id,
                            progress: 0.0,
                            status: "cancelled",
                        },
                    );
                    return Ok(ConvertResult { cancelled: true, success: false, output_path: String::new(), target_format, error: None });
                }
            }
        }
    };
    let event_app = app.clone();
    let event_job = job_id.clone();
    let last_emit = Mutex::new(std::time::Instant::now() - std::time::Duration::from_secs(1));
    let progress = move |p: f32| {
        let mut last = last_emit.lock().unwrap_or_else(|p| p.into_inner());
        if !p.is_finite() || last.elapsed() < std::time::Duration::from_millis(50) {
            return;
        }
        *last = std::time::Instant::now();
        let _ = event_app.emit(
            "convert_progress",
            ProgressPayload {
                job_id: &event_job,
                progress: p.clamp(0.0, 0.99),
                status: "converting",
            },
        );
    };
    let result = tokio::task::spawn_blocking(move || {
        // Ownership follows the actual worker even if the IPC future is dropped.
        let _permit = permit;
        let _guard = guard;
        registry.convert(&input, target_format, &opts, &progress, &cancel)
    })
    .await;
    let result = match result {
        Ok(result) => result,
        Err(error) => Err(vertex_core::VertexError::Internal(error.to_string())),
    };
    let (success, cancelled, output_path, error) = match result {
        Ok(path) => (
            true,
            false,
            vertex_core::path_utils::external_path(&path)
                .to_string_lossy()
                .into_owned(),
            None,
        ),
        Err(error) => (
            false,
            matches!(error, vertex_core::VertexError::Cancelled),
            String::new(),
            Some(error.to_string()),
        ),
    };
    let _ = app.emit(
        "convert_progress",
        ProgressPayload {
            job_id: &job_id,
            progress: if success { 1.0 } else { 0.0 },
            status: if success {
                "done"
            } else if cancelled {
                "cancelled"
            } else {
                "error"
            },
        },
    );
    Ok(ConvertResult {
        success,
        cancelled,
        output_path,
        target_format,
        error,
    })
}

#[tauri::command]
pub async fn cancel_job(job_id: String, state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let map = state
        .active_cancels
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(token) = map.get(&job_id) {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    let p = vertex_core::path_utils::ipc_path(&path)?;
    let safe_path = vertex_core::security::SafeLauncher::sanitize_explorer_path(&p)?;

    tauri_plugin_opener::reveal_item_in_dir(&safe_path)
        .map_err(|e| format!("Failed to reveal file: {e}"))
}

// Native input/visibility changes and their generation checks must execute in
// one UI-thread operation. A check on a worker followed by a queued hide can
// otherwise act on a newer Settings window.
async fn on_main_window(
    app: AppHandle,
    action: impl FnOnce(tauri::WebviewWindow) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let ui_app = app.clone();
    app.run_on_main_thread(move || {
        let result = match ui_app.get_webview_window("main") {
            Some(window) => action(window),
            None => Err("Main window is unavailable".into()),
        };
        let _ = sender.send(result);
    })
    .map_err(|e| e.to_string())?;
    receiver.await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn hide_wheel_window(app: AppHandle, generation: u32) -> Result<(), String> {
    on_main_window(app, move |win| {
        if crate::drag_detector::settings_open() || generation != crate::wheel_window::generation()
        {
            return Ok(());
        }
        crate::wheel_window::hide(&win)
    })
    .await
}

#[tauri::command]
pub async fn force_hide_wheel_window(app: AppHandle) -> Result<(), String> {
    on_main_window(app, |win| {
        if crate::drag_detector::settings_open() {
            return Ok(());
        }
        crate::wheel_window::hide(&win)
    })
    .await
}

#[tauri::command]
pub fn set_converting_state(converting: bool) {
    crate::drag_detector::set_is_converting(converting);
}

/// Compatibility endpoint for frontend capture requests. Visible windows always
/// capture input; hiding the HWND is the only way to pass input to the desktop.
#[tauri::command]
pub async fn set_window_passthrough(app: AppHandle, passthrough: bool) -> Result<(), String> {
    on_main_window(app, move |win| {
        crate::wheel_window::set_passthrough(&win, passthrough)
    })
    .await
}

/// Center the main window on the screen
#[tauri::command]
pub fn center_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.center().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Prepare window for settings: clear passthrough, resize with 80px margins, center, show, and focus
#[tauri::command]
pub async fn prepare_settings_window(app: AppHandle) -> Result<(), String> {
    on_main_window(app, |win| {
        crate::wheel_window::invalidate_close();
        let monitor = win.current_monitor().map_err(|e| e.to_string())?;
        let (width, height) = monitor
            .map(|m| {
                let work = m.work_area();
                (
                    1080.0f64.min(f64::from(work.size.width) / m.scale_factor()),
                    780.0f64.min(f64::from(work.size.height) / m.scale_factor()),
                )
            })
            .unwrap_or((1080.0, 780.0));
        win.set_size(tauri::LogicalSize::new(width, height))
            .map_err(|e| e.to_string())?;
        crate::wheel_window::show(&win, None, true)?;
        crate::drag_detector::set_settings_open(true);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn restore_wheel_window(app: AppHandle) -> Result<(), String> {
    crate::drag_detector::set_settings_open(false);
    on_main_window(app, |win| {
        crate::drag_detector::set_settings_open(false);
        crate::wheel_window::invalidate_close();
        crate::wheel_window::hide(&win)?;
        win.set_size(tauri::LogicalSize::new(480.0, 480.0))
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn job_lifetime_preserves_busy_state_and_cleans_up_after_a_panic() {
        crate::drag_detector::set_is_converting(false);
        let cancels = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let token = CancelToken::new();
        let first = JobGuard::register("first", &token, &cancels).unwrap();
        let second = JobGuard::register("second", &token, &cancels).unwrap();
        assert!(JobGuard::register("second", &token, &cancels).is_err());
        drop(first);
        crate::drag_detector::set_is_converting(false);
        assert!(crate::drag_detector::get_is_converting());
        let result = std::panic::catch_unwind(|| {
            let _second = second;
            panic!("test worker unwind");
        });
        assert!(result.is_err());
        assert!(cancels.lock().unwrap().is_empty());
        assert!(!crate::drag_detector::get_is_converting());
    }
}
