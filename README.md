<p align="center">
  <img src="frontend/vertex.png" width="100" height="100" alt="Vertex">
</p>

<h2 align="center">Vertex</h2>

<p align="center">
  Minimalist radial wheel file converter for Windows
</p>

<p align="center">
  <a href="https://github.com/rainaku/Vertex/releases/latest">
    <img src="https://img.shields.io/github/v/release/rainaku/Vertex?style=for-the-badge&color=ffffff&labelColor=eeeeee&logo=github&logoColor=111111" alt="Latest Release">
  </a>
  <img src="https://img.shields.io/badge/Windows-10%20%7C%2011-ffffff?style=for-the-badge&labelColor=eeeeee&logo=windows&logoColor=111111" alt="Windows 10 / 11">
  <a href="LICENSE">
    <img src="https://img.shields.io/github/license/rainaku/Vertex?style=for-the-badge&color=ffffff&labelColor=eeeeee" alt="License">
  </a>
</p>

<p align="center">
  <a href="#usage">Usage</a> ·
  <a href="#supported-formats">Supported formats</a> ·
  <a href="#installation">Installation</a> ·
  <a href="#license">License</a>
</p>

<p align="center">
  Vertex turns file conversion into a single gesture. Hold Shift while dragging any file in Windows to summon a radial wheel at your cursor, then drop the file onto the format you want.
</p>

<p align="center">
  <b>Free and open source.</b>
</p>

<p align="center">
  Vertex is an independent project.<br>
  If you find it useful, you can <a href="https://www.paypal.me/PhuocLe678"><b>support development via PayPal</b></a>.
</p>

<div id="usage"></div>

## Usage

| Action | Result |
| --- | --- |
| `Shift + Drag` a file | Open the radial wheel at cursor position |
| Hover over a slice | Highlight the target format |
| Hover on `...` (~250ms) | Go to the next page of formats |
| Drop onto a format slice | Start conversion immediately |
| Drop onto `...` slice | Go to the next page of formats |
| Release Shift or drag away | Close the wheel without converting |

Converted files are saved automatically in the same folder as the original file.

<div id="supported-formats"></div>

## Supported formats

| Category | Formats | Notes |
| --- | --- | --- |
| Video | MP4, MOV, MKV, AVI, WEBM, WMV, FLV, 3GP, TS, GIF | Includes MP4 to MP4 compression to reduce file size |
| Audio | MP3, WAV, FLAC, AAC, M4A, OGG, OPUS, AIFF, WMA | Convert music or extract audio directly from video files |
| Image | PNG, JPEG, WEBP, BMP, ICO, TIFF, QOI, AVIF | Fast conversion across popular photo and icon formats |

<div id="installation"></div>

## Installation

Vertex runs on 64-bit Windows 10 and Windows 11.

1. Download the latest installer from [Releases](https://github.com/rainaku/Vertex/releases).
2. Run the installer and launch Vertex.
3. Hold `Shift` while dragging any file to start converting.

> [!NOTE]
> Video and audio conversion requires [FFmpeg](https://ffmpeg.org/download.html) on your system.

<details>
<summary><strong>Build from source</strong></summary>

Prerequisites: Rust (latest stable toolchain), Node.js 18+.

1. Clone the repository:
   ```bash
   git clone https://github.com/rainaku/Vertex.git
   cd Vertex
   ```

2. Install frontend dependencies:
   ```bash
   cd frontend
   npm install
   cd ..
   ```

3. Launch in development mode using PowerShell:
   ```powershell
   ./r
   ```
   Or run Tauri directly:
   ```bash
   cargo tauri dev
   ```

4. Build the release binary and installer:
   ```bash
   cargo tauri build
   ```
   The compiled installer will be located in `target/release/bundle/`.

</details>

<div id="license"></div>

## License

Vertex is licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details.

<p align="center">
  Made by <a href="https://rainaku.id.vn">rainaku</a> ·
  <a href="https://github.com/rainaku/Vertex">GitHub</a> ·
  <a href="https://www.facebook.com/rain.107/">Facebook</a> ·
  <a href="https://www.paypal.me/PhuocLe678">Donate</a>
</p>
