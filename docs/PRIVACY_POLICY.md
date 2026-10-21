# Privacy Policy — Vertex

**Effective date:** October 1, 2026  
**Maintainer:** rainaku  
**Project:** https://github.com/rainaku/Vertex  
**Contact:** [github.com/rainaku/Vertex/issues](https://github.com/rainaku/Vertex/issues)  

---

## 1. Overview

Vertex is a free, open-source radial wheel file converter for Windows. This Privacy Policy details what data the application accesses on your computer, how that data is handled, and where network connections occur.

**Core privacy principle:** Vertex is local-first. The application has no tracking, no user accounts, no telemetry, no advertisements, and no analytics. It runs no remote backend servers. Your files are converted entirely on your local machine and never leave your device.

---

## 2. Summary at a Glance

| Capability | What it accesses | Leaves your device? | Stored on disk? |
|---|---|---|---|
| **File conversion** | File paths and file contents selected by dragging | **No** (Processed 100% locally via Rust core and FFmpeg) | Yes, only as output files in your chosen destination directory |
| **Shift + Drag gesture** | Mouse cursor position and Shift key state | **No** | No (Transient in memory) |
| **Settings & Preferences** | Language choice, quality presets, custom DPI, theme | **No** | Yes (Stored locally in WebView2 local storage) |
| **Update check (Auto/Manual)** | Current version tag sent in standard HTTP header | **Yes** — GitHub Releases API (`api.github.com` / `github.com`) | Temporary installer cache in `%TEMP%` when you choose to update |
| **External links** | Browser launch requests (GitHub, PayPal, Website) | **Yes** — Directed only to the destination URLs you click | Standard browser history governed by your browser |

---

## 3. Data Accessed on Your Device

### 3.1 File Processing
When you hold `Shift` and drag files over the radial wheel:
- Vertex reads the file path to inspect file signatures (magic bytes) locally via the `infer` crate and extension checks.
- When you drop onto a target slice, file contents are streamed directly to local conversion encoders (in-memory image codecs or a local `ffmpeg.exe` process).
- Output files are written to the target folder you specify (by default, the source file's directory).
- File data is never sent over any network socket or uploaded to any remote host.

### 3.2 Keyboard and Mouse Hooks
- Vertex listens for the `Shift` key and left mouse button state using low-level Windows APIs (`SetWindowsHookExW` and `GetAsyncKeyState`).
- These events are processed strictly to detect the drag-and-drop gesture and place the radial wheel at your cursor position.
- Keystrokes are not recorded, logged, or stored.

### 3.3 Application Settings
- Settings such as display language (`vi` / `en`), image quality sliders, video compression preferences, and DPI defaults are stored locally in your local profile storage via the Microsoft WebView2 cache.
- These settings never leave your machine.

---

## 4. Network Connections

The only outbound network requests initiated by Vertex are:

1. **Update checks:**
   - **Endpoint:** `https://github.com/rainaku/Vertex/releases/latest/download/latest.json` and `https://api.github.com/repos/rainaku/Vertex/releases/latest`.
   - **Payload:** Standard HTTP GET request. No telemetry, machine identifiers, or personal data are included.
   - **Frequency:** An initial check 15 seconds after startup, recurring every 6 hours, or when you click "Check for Updates" in the Settings panel.
2. **Update downloads:**
   - When you confirm an update, the installer binary is downloaded directly from GitHub Releases into your system temporary folder (`%TEMP%`) and verified with cryptographic signatures before execution.
3. **Outbound link clicks:**
   - Clicking on social, repository, or donation links opens your default system browser to the target site (e.g., GitHub, PayPal).

---

## 5. Third-Party Integrations and Subprocessors

- **FFmpeg:** Audio and video conversions invoke your locally installed FFmpeg binary. FFmpeg runs as an isolated child process on your computer without network transmission.
- **GitHub:** Hosts the project releases and updater manifest. Your interactions with GitHub are governed by GitHub's Privacy Statement.
- **Microsoft WebView2:** Vertex uses the Windows WebView2 runtime to render its radial UI. Microsoft's runtime adheres to system-level Windows telemetry policies.

---

## 6. Data Retention and Deletion

- Converted files remain on your disk until you delete them.
- Uninstalling Vertex removes the application files and registers. To remove local preferences, clear the application directory in `%LOCALAPPDATA%\com.vertex.converter`.

---

## 7. Your Rights

Because Vertex does not collect, transmit, or store personal data on any server, there is no remote database to query, export, or erase under GDPR, CCPA, or regional data protection laws. You maintain absolute control over all data on your local device.

---

## 8. Changes to this Policy

Any updates to this policy will be committed directly to the official GitHub repository with a revised effective date.

---

## 9. Contact

If you have questions or concerns regarding privacy in Vertex, open an issue on GitHub:  
https://github.com/rainaku/Vertex/issues
