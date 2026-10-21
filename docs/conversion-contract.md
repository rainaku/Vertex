# Conversion contract and engines

The Rust registry owns supported inputs, targets and file extensions. The desktop file picker calls `get_input_formats`; per-source target lists and dependency availability come from `get_available_targets` / `get_batch_info`. Missing tools leave targets visible but disabled, with the actual dependency reason shown in the wheel.

`crates/core/src/contract.rs` also exports option defaults, numeric bounds and route notices. `frontend/src/lib/conversion-contract.json` is the checked-in snapshot used by settings and browser development mode. Regenerate it with:

```powershell
cargo run -p vertex-core --example export_contract
```

Rust tests compare the entire snapshot with the live registry. Frontend tests exercise its aliases, routes, option bounds and shared settings controls. CI runs both suites. Browser conversions remain simulations; real file conversions run in the desktop backend.

## Conversion routes

| Inputs | Outputs | Engine |
| --- | --- | --- |
| PNG, JPG, WebP, BMP, ICO, TIFF, GIF | PNG, JPG, WebP, AVIF, BMP, ICO, TIFF, GIF | Rust image encoder |
| AVIF | Same raster outputs | FFmpeg, or ImageMagick with an AVIF decoder |
| HEIC/HEIF | Same raster outputs | ImageMagick with a HEIC decoder (libheif) |
| SVG | Same raster outputs | resvg; DPI applies to physical units such as inches |
| PDF | Same raster outputs; TXT | Poppler `pdftoppm` / `pdftotext` |
| PDF | DOCX with editable paragraphs and tables | Local `pdf2docx` CLI |
| DOC, DOCX, DOCM, DOT, DOTX, DOTM, ODT, OTT, FODT, SXW, WPS, WPD, WRI, RTF, TXT, HTML/XHTML, Markdown | DOC, DOCX, DOTX, ODT, OTT, FODT, RTF, TXT, HTML, PDF (excluding identical office formats) | LibreOffice; Markdown first becomes UTF-8 HTML |
| Markdown | HTML, TXT | Rust Markdown parser |
| TXT | HTML | Rust escaping and UTF-8 HTML |
| XLSX, XLS, ODS, CSV, TSV | Other formats in this group, PDF | LibreOffice; native Rust handles CSV/TSV pairs |
| PPTX, PPT, ODP | Other presentation formats, PDF | LibreOffice |
| JSON, YAML, TOML, XML, CSV, TSV | Formats in this group | Rust structured-data converters |
| ZIP, TAR.GZ/TGZ, 7Z, GZ | ZIP, TAR.GZ, 7Z | Rust archive readers/writers |
| Audio, video, GIF | Existing audio/video routes | FFmpeg |

HEIC and SVG are input-only. PDF supports rendering, text extraction and editable Word reconstruction. Raster inputs do not become vector SVGs. Bare `.gz` contains one payload; `.tar.gz` and `.tgz` preserve archive entries. Gzip output is not offered for archives containing multiple files.

## Options and output semantics

- `quality` applies to JPG and AVIF. WebP remains lossless; PNG/TIFF do not use this quality value. `avif_speed` applies to the Rust AVIF encoder.
- `dpi` is used by PDF rasterization and by SVG physical-unit conversion. PDF exports the first page; PDF-to-TXT extracts the text layer from all pages and does not perform OCR.
- PDF-to-DOCX reconstructs paragraphs, tables and images from all pages using pdf2docx. It does not rasterize pages or use image DPI/resize/quality controls. Fonts and complex layouts may differ; install the original fonts for closer results. Images (including scanned text) remain images; there is no OCR. Image-only/empty Word output is rejected with an OCR message. In mixed PDFs, scanned pages remain non-editable images. Page errors abort the whole conversion, rather than silently dropping failed pages. Jobs have a five-minute timeout and publish only after checking the DOCX text layer.
- DOC/DOCX/ODT/RTF conversions preserve the features supported by both formats; exact layout equality is not guaranteed. TXT exports discard fonts, colors, images and table formatting. These limitations are shown in the wheel before conversion.
- DOTX and OTT exports create reusable document templates; FODT is flat XML OpenDocument text. DOCM, DOT, DOTM, SXW, WPS (Microsoft Works), WPD (WordPerfect) and WRI (Microsoft Write) are input-only. Macro-enabled Word inputs export document content with macro execution disabled; preserving macros is not supported. Legacy format features depend on the installed LibreOffice import filters. The original file is retained.
- Word ZIP packages are classified by their main-part content type, distinguishing DOCX/DOCM/DOTX/DOTM even after renaming. OTT and SXW use the package MIME type. Flat ODF XML is recognized as FODT rather than structured-data XML. NFO/ASC are plain-text aliases, MDOWN/MKD/MDWN are Markdown aliases, and XHTML/XHT use HTML handling; NFO/ASC still require supported Unicode encoding.
- TXT/Markdown input accepts strict UTF-8 (with or without a BOM) and UTF-16LE/BE with a BOM. It retains Unicode, tabs and blank lines. Invalid/ambiguous encodings, including legacy ANSI and BOM-less UTF-16 containing NULs, fail with an encoding message instead of replacing characters. Save such files as UTF-8 first. Text input is limited to 64 MiB.
- Maximum dimensions apply to raster images, PDF/SVG renders and video. ICO has its own 256-pixel format limit. Raster conversions use the first image/frame for animated or multipage image inputs.
- JPEG background fill replaces transparency. GIF alpha threshold applies to still-image GIF output and to FFmpeg GIF palette conversion.
- Metadata stripping is an image/media option. Preservation depends on the source decoder and output format; AVIF decoding through FFmpeg does not retain the original EXIF/ICC metadata. Office exports use the corresponding LibreOffice export filter.
- H.264/H.265 codec and encoding preset apply to compatible video containers. The video audio bitrate control is in Video & Audio, with the backend's 64–320 kbps bounds. Format-specific codecs can impose their own settings.
- XLSX/XLS/ODS export to CSV/TSV uses the first sheet and evaluated values. CSV/TSV cannot retain workbook formulas, formatting or multiple sheets. HTML Office export embeds images so moving the single output file does not discard image sidecars.
- CSV/TSV use UTF-8 and proper quoting, including embedded separators and newlines. JSON table output is an array of objects with string cell values. Data-to-CSV/TSV requires an array of objects with scalar cells; nested data and duplicate column names are rejected. Column names are sorted on export from structured data; empty tables do not carry a column schema.
- XML maps attributes to `@name`, text to `$text`, repeated elements to arrays, and keeps the root name. Scalars become strings, reflecting XML's data model. Structured objects with several root keys use `<root>`; top-level arrays use `<root><item>…</item></root>`. Invalid XML names, DTDs and mixed text/child content are rejected. TOML-inexpressible values such as null are rejected instead of silently changed. Structured-data inputs are limited to 64 MiB.
- Archive repacking retains names, bytes and empty directories. Links, special files, unsafe paths and case-insensitive duplicate paths are rejected. Archive ownership, timestamps and permissions are not preserved. Expanded input is capped at 8 GiB and 100,000 entries.
- Converters stage output before publishing. Failed or cancelled conversions clean up temporary work. Compound suffixes such as `.tar.gz` are retained when staging. Output directory file URLs are decoded and checked at the IPC boundary.

## Optional local tools

Place tools in `PATH`, or `bin` next to the Vertex executable. Standard LibreOffice and ImageMagick installation directories on Windows are also detected. No tool is downloaded or installed automatically.

- [LibreOffice command-line parameters](https://help.libreoffice.org/latest/en-US/text/shared/guide/start_parameters.html) and [conversion filters](https://help.libreoffice.org/latest/en-US/text/shared/guide/convertfilters.html). Jobs use separate temporary profiles to avoid conflicts with a running GUI or other conversions. Macros are disabled. The process tree is terminated on timeout/cancellation on Windows.
- [LibreOffice CSV filter parameters](https://help.libreoffice.org/latest/en-US/text/shared/guide/csv_params.html) define UTF-8, explicit delimiters and first-sheet export.
- [Poppler](https://poppler.freedesktop.org/) provides `pdftoppm` and `pdftotext`.
- [pdf2docx CLI](https://pdf2docx.readthedocs.io/en/stable/quickstart.cli.html) provides editable PDF-to-DOCX reconstruction. On Windows, run `.\scripts\Install-DocumentTools.ps1` once with Python installed (or pass `-Python C:\path\to\python.exe`). This explicit setup installs pdf2docx 0.5.13 into `%USERPROFILE%\.vertex\tools\pdf2docx`, which Vertex detects without changing `PATH`. Alternatively install with `python -m pip install pdf2docx` and put its executable directory (usually Python's `Scripts` directory on Windows) on `PATH`. Vertex invokes `pdf2docx convert` with `--ignore_page_error=False --raw_exceptions=True --multi_processing=False`. The app does not install packages automatically. Conversions run locally.
- [ImageMagick supported formats](https://imagemagick.org/formats/) describe the required delegates. Vertex probes readable AVIF/HEIC support, rather than assuming the executable includes those decoders.
- [FFmpeg](https://ffmpeg.org/download.html) provides media conversion and AVIF decoding.

Run `npm test`, `npm run check`, `npm run build` in `frontend`, and `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings` at the repository root. AVIF/PDF integration tests run when FFmpeg/Poppler are installed. Editable Word integration runs when `pdf2docx` is on `PATH` (verified with pdf2docx 0.5.13); it checks text, tables, font styling, mixed page sizes, cancellation and failed-output protection. LibreOffice and ImageMagick paths require installations with the relevant codecs for end-to-end verification.
