//! Shared lifecycle for optional local conversion tools. No shell is involved.
use crate::{CancelToken, Result, VertexError};
use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

type ToolCache = Mutex<HashMap<String, (Instant, Option<PathBuf>)>>;
static TOOL_CACHE: OnceLock<ToolCache> = OnceLock::new();

pub fn find_tool(name: &str, locations: &[&str]) -> Option<PathBuf> {
    let key = format!("{name}\0{}", locations.join("\0"));
    let mut cache = TOOL_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some((checked, path)) = cache.get(&key) {
        if path.as_ref().is_some_and(|p| p.is_file())
            || (path.is_none() && checked.elapsed() < Duration::from_secs(2))
        {
            return path.clone();
        }
    }
    let found = find_tool_uncached(name, locations);
    cache.insert(key, (Instant::now(), found.clone()));
    found
}

fn find_tool_uncached(name: &str, locations: &[&str]) -> Option<PathBuf> {
    let exe = if cfg!(windows) && Path::new(name).extension().is_none() {
        format!("{name}.exe")
    } else {
        name.into()
    };
    let mut candidates = Vec::new();
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(
            std::env::split_paths(&paths)
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.join(&exe)),
        );
    }
    if let Ok(current) = std::env::current_exe() {
        if let Some(base) = current.parent() {
            candidates.push(base.join("bin").join(&exe));
            candidates.push(base.join(&exe));
        }
    }
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(variable) {
            candidates.extend(
                locations
                    .iter()
                    .map(|location| PathBuf::from(&base).join(location).join(&exe)),
            );
            if name == "magick" {
                if let Ok(entries) = std::fs::read_dir(&base) {
                    candidates.extend(
                        entries
                            .flatten()
                            .filter(|e| e.file_name().to_string_lossy().starts_with("ImageMagick-"))
                            .map(|e| e.path().join(&exe)),
                    );
                }
            }
        }
    }
    candidates.extend(
        locations
            .iter()
            .filter(|p| Path::new(p).is_absolute())
            .map(|p| Path::new(p).join(&exe)),
    );
    candidates
        .into_iter()
        .find(|p| {
            if !p.is_file() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                p.metadata()
                    .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
            }
            #[cfg(not(unix))]
            true
        })
        .and_then(|p| std::path::absolute(p).ok())
}

struct ChildGuard {
    child: Child,
    #[cfg(windows)]
    job: Option<ProcessJob>,
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        drop(self.job.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// LibreOffice and codec delegates spawn children. Closing the job terminates
/// that entire process tree before its temporary workspace is removed.
#[cfg(windows)]
struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl ProcessJob {
    fn attach(child: &Child) -> std::io::Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let job = Self(handle);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(job)
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// Redirect both streams to files so a verbose tool cannot deadlock a full pipe.
pub fn run_tool(cmd: &mut Command, cancel: &CancelToken, timeout: Duration) -> Result<String> {
    cancel.check()?;
    let mut log = tempfile::tempfile().map_err(|e| VertexError::Encoding(e.to_string()))?;
    cmd.stdin(Stdio::null())
        .stdout(
            log.try_clone()
                .map_err(|e| VertexError::Encoding(e.to_string()))?,
        )
        .stderr(
            log.try_clone()
                .map_err(|e| VertexError::Encoding(e.to_string()))?,
        );
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = ChildGuard {
        child: cmd
            .spawn()
            .map_err(|e| VertexError::Encoding(format!("Could not start conversion tool: {e}")))?,
        #[cfg(windows)]
        job: None,
    };
    #[cfg(windows)]
    {
        child.job = Some(ProcessJob::attach(&child.child).map_err(|e| {
            VertexError::Encoding(format!("Could not contain conversion process: {e}"))
        })?);
    }
    let started = Instant::now();
    let status = loop {
        cancel.check()?;
        if let Some(status) = child
            .child
            .try_wait()
            .map_err(|e| VertexError::Encoding(e.to_string()))?
        {
            break status;
        }
        if started.elapsed() >= timeout {
            return Err(VertexError::Encoding("Conversion tool timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    log.rewind()
        .map_err(|e| VertexError::Encoding(e.to_string()))?;
    let mut bytes = Vec::new();
    log.take(128 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| VertexError::Encoding(e.to_string()))?;
    let message = String::from_utf8_lossy(&bytes).into_owned();
    if !status.success() {
        return Err(VertexError::Encoding(format!(
            "Conversion tool exited with {status}: {message}"
        )));
    }
    cancel.check()?;
    Ok(message)
}

pub fn io_error(path: &Path, source: std::io::Error) -> VertexError {
    VertexError::Io {
        path: path.to_path_buf(),
        source,
    }
}

pub fn workspace() -> Result<tempfile::TempDir> {
    tempfile::tempdir().map_err(|e| io_error(&std::env::temp_dir(), e))
}

pub fn copy_output(input: &Path, output: &Path, cancel: &CancelToken) -> Result<()> {
    cancel.check()?;
    let staged = crate::path_utils::stage_output(output)?;
    std::fs::copy(input, &staged).map_err(|e| io_error(output, e))?;
    cancel.check()?;
    crate::path_utils::publish_exact(staged, output)
}
