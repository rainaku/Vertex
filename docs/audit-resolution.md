# Kết quả xử lý audit Vertex — 16/10/2026

Đã đối chiếu các mã trong `audit_and_improvements.md` với code hiện tại và sửa các luồng liên quan. Những thay đổi có sẵn trong working tree được giữ và hoàn thiện. Bảng này phân biệt thay đổi đã triển khai với các cấu hình phần cứng chưa thể kiểm tra trực tiếp.

## Thay đổi theo mã audit

| Mã | Giải pháp trong code |
| --- | --- |
| DET-01 | Đọc root element bằng quick-xml trước infer; hỗ trợ BOM, XML declaration và comment trước SVG. |
| DET-02 | Đọc cấu trúc ZIP qua central directory để phân biệt OOXML/ODF; đọc EBML DocType; phân biệt brand ISO-BMFF; bỏ qua ID3v2 theo kích thước syncsafe; disambiguate ASF và Ogg trong cùng họ container. |
| DET-03 | Nhận suffix `.tar.gz`/`.tgz` và OpusHead; bỏ lớp MIME suy đoán từ đuôi vốn ghi đè định dạng chính xác hơn. |
| DET-04 | Chấp nhận prefix UTF-8 kết thúc giữa codepoint, bỏ BOM, so sánh HTML không phân biệt hoa thường; validate JSON bằng streaming `IgnoredAny`, không tạo DOM hoặc giới hạn JSON ở 8 KB. |
| DET-05 | Mở file một lần, giữ nguyên lỗi I/O, kiểm tra regular file, không nuốt lỗi đọc hoặc coi thư mục là ảnh. |
| FMT-01 | Wire label `3GP`, `TAR.GZ`, `7Z` thống nhất với frontend; giữ alias tương thích dữ liệu cũ. |
| FMT-02 | Chuẩn hóa ASCII trong buffer stack, tách tham số MIME. |
| FMT-03 | Bổ sung MIME của OOXML/ODF, RTF, TSV, M4A, M4V, 7z và các alias khác. |
| OPT-01 | `Options::normalized()` dùng chung ở registry và converter: bỏ thư mục trắng, bỏ kích thước 0, giới hạn quality/DPI/codec options. |
| PTH-01 | `parent_dir()` chuẩn hóa parent rỗng thành `.`. |
| PTH-02 | Xuất vào file tạm riêng; công bố nguyên tử bằng no-clobber và thử tên mới khi va chạm. Không dùng kiểm tra tồn tại làm bảo đảm đặt tên, không mã hóa lại khi bị tranh tên. |
| PTH-03 | Làm sạch stem, tên thiết bị Windows, dấu chấm/khoảng trắng cuối; xử lý compound extension. |
| PTH-04 | Không canonicalize file đích chưa tồn tại; dùng identity của file, gồm symlink/hardlink, để bảo vệ input. |
| REG-01 | Registry detect source một lần, truyền DetectedInput qua từng hop, lập route trước khi tạo đầu ra. FFmpeg chỉ tự detect khi gọi converter trực tiếp. |
| REG-02 | Chỉ chọn converter khả dụng; cho phép fallback; trả EngineMissing trước khi tạo thư mục/file tạm. |
| REG-03 | BFS lưu predecessor thay vì clone mọi path; sort_unstable_by_key với thứ tự xác định. |
| CAN-01 | Atomic Relaxed cho flag độc lập không công bố dữ liệu khác; inline các thao tác hủy. |
| CRG-01 | Bật rayon/threading cho image/ravif; dùng fast_image_resize 6.1 với SIMD dispatch, alpha-aware convolution và rayon. |
| FFMPEG-01 | Cache thành công có kiểm tra file; kết quả thiếu binary có TTL 2 giây, không bị lưu vĩnh viễn. |
| FFMPEG-02 | Trả đường dẫn tuyệt đối; kiểm tra is_file; hỗ trợ Scoop, WinGet không phân biệt hoa thường và các vị trí cài phổ biến. |
| FFMPEG-03 | Probe dùng `-i`, stdin null, cả format/stream duration, lọc số không hữu hạn; có thời hạn và kiểm tra hủy. |
| FFMPEG-04 | Đường dẫn truyền qua Path/OsStr; chỉ bỏ prefix local verbatim khi gọi API ngoài. |
| FFMPEG-05 | Chặn input/output cùng identity trước khi chạy; converter trực tiếp cũng encode vào staging, không ghi đè input. |
| FFMPEG-06 | Luôn map audio đầu tiên và bỏ video/subtitle/data khi xuất audio; stereo/48 kHz cho MP3/WMA; 48 kHz cho Opus. |
| FFMPEG-07 | yuv420p, scale với kích thước chẵn; H.263 fit+pad vào CIF; VP9 dùng quality deadline và CRF người dùng. |
| FFMPEG-08 | TempPath sở hữu cleanup; ChildGuard kill+wait khi lỗi, hủy hoặc unwind. Không xóa output có sẵn khi encode lỗi. |
| FFMPEG-09 | Reader tái sử dụng line buffer và chỉ gửi số thời gian; stderr bounded tail, retry Interrupted, giữ nguyên thông báo nguyên nhân. |
| IMAGE-01 | ImageReader đoán theo nội dung, decoder.orientation và apply_orientation trước resize. |
| IMAGE-02 | Resize giữ bit depth, alpha-aware bằng SIMD; một phép tính kích thước và một lần resize gồm cả giới hạn ICO. |
| IMAGE-03 | BufWriter flush được kiểm tra; staging được dọn trên lỗi/hủy; chỉ commit sau thành công. |
| IMAGE-04 | Blend pixel tuyến tính, giữ RGB khi không cần alpha, GIF speed 10; chuẩn hóa float sang PNG 16-bit. |
| SLP-01 | Kiểm tra reparse tag; chỉ theo dõi name-surrogate và xác minh target local. Cloud/dedup tags được phép; đọc metadata bằng handle không yêu cầu đọc nội dung. |
| SLP-02 | Phân tích Prefix của Path, phân biệt local verbatim với UNC/device paths; gỡ prefix cho API ngoài. |
| SLP-03 | URL parser và allowlist scheme, kiểm tra host/userinfo/control characters. |
| INT-01 | `about:internet` là origin hint không xác định; vẫn kiểm tra URL nguồn/referrer thực nếu có. |
| INT-03 | Khóa INI không phân biệt hoa thường, chấp nhận whitespace; ADS tối đa 64 KB; hash buffer heap và sequential scan. Không cache hash chỉ theo mtime/size vì nội dung có thể đổi mà hai giá trị này giữ nguyên. |
| SCR-01 | Một regex tổng hợp với quoted escapes và các khóa unquoted; Cow mượn input khi log không có secret. |
| CMD-01 | Semaphore giới hạn worker theo CPU (1–2); token đăng ký trước khi xếp hàng; JobGuard giữ refcount và dọn map khi worker kết thúc/unwind. Frontend không thể tắt trạng thái của các worker còn chạy. |
| CMD-02 | Progress IPC tối đa 20 Hz/job, dùng payload mượn chuỗi; terminal event gửi riêng. |
| CMD-03 | Native reveal_item_in_dir thay cho chuỗi tham số explorer.exe. Script build dùng SHOpenFolderAndSelectItems. |
| CMD-04 | Chuẩn hóa file URL tại IPC bằng URL parser; Settings giới hạn kích thước theo work area/DPI; panel responsive. |
| DRG-01 | Chặn gesture trong Settings, khi worker/UI đang bận và khi foreground fullscreen; loại trừ desktop Explorer. |
| DRG-02 | Chọn monitor theo con trỏ; chuyển cửa sổ ẩn sang monitor đích, đọc GetDpiForWindow rồi tính kích thước/toạ độ trong rcWork. |
| DRG-03 | Primary button theo SM_SWAPBUTTON; state machine chặn mở lại sau Esc cho tới release; kiểm tra GetCursorPos khi nhấn Shift giữa một drag. |
| DRG-04 | WH_MOUSE_LL/WH_KEYBOARD_LL + message loop và channel thay cho polling 15 ms; chỉ theo dõi motion khi đang chờ gesture; bỏ event alias trùng. |
| WW-01 | DWM thiết lập một lần; frame refresh chỉ khi đổi tool/app style, không khi bật/tắt click-through; focus không bị NOACTIVATE; Rust là nơi duy nhất resize/show Settings; tray summon center lại. |
| CFG-01 | Release thin LTO, một codegen unit, strip symbols; giữ unwind để RAII hoạt động khi worker panic. Cửa sổ không resizable; bổ sung CSP asset/GitHub. |
| WHL-01 | DOM pointer và native drag cùng dùng hitTestClient, chuyển từ bounding rectangle đã transform sang viewBox. |
| WHL-02 | Bỏ shadow filter trên container động; Settings dùng box-shadow; geometry của tám petal tính trước. |
| APP-01 | Snapshot batch, chuyển toàn bộ file lần lượt, gom lỗi và dừng file chưa chạy khi hủy; Settings invalidate exit; callback trả đúng index bàn phím; sở hữu và dọn listener/timer kể cả đăng ký async. |
| SET-01 | Giữ DPI và strip_metadata trong normalization/persistence; khi giữ metadata, JPEG/PNG/WebP giữ EXIF/ICC, TIFF giữ ICC. EXIF Orientation được reset sau khi đã xoay pixel. |
| UPD-01 | Plugin trả null thì kết thúc với up-to-date; fallback chỉ khi check lỗi; dùng SemVer chuẩn, native app version, single-flight và fetch timeout. |
| OPT-02 | Search khai báo dependency reactive rõ ràng; preset/shortcuts dùng chung Svelte snippet; một handler kéo panel; slider cache rectangle theo gesture và snap từ min; keyframe tooltip riêng; loại font-face trùng. |
| BRI-01 | Resize/show Settings một lần qua Rust; picker có đủ nhóm format; hit test dùng khoảng cách bình phương và kết quả cache. |
| PS-01 | JSON UTF-8 không BOM trên PS 5.1/7; native file selection; TCP probe nhanh; Vite chạy trực tiếp qua node và dọn process tree được sở hữu; mọi lệnh cargo chạy trong workspace với try/finally. Recursive cleanup kiểm tra phạm vi workspace. |

## Bằng chứng kiểm tra

- 42 bài kiểm tra Rust trong workspace: unit, tích hợp ảnh/container, concurrency, security, lifecycle của worker/gesture/OLE drop, và FFmpeg thật.
- 31 bài kiểm tra frontend: geometry có transform, batch/cancel, settings, SemVer, native updater trả null, keyboard selection, search thực trong Svelte, Settings mở trong lúc wheel đang đóng và các nhánh kéo thả bên dưới.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo fmt --all -- --check`.
- `npm run check`, `npm test`, `npm run build`, `npm audit --audit-level=high` (không phát hiện lỗ hổng).
- `tests/verify_scripts.ps1` chạy bằng PowerShell 5.1 và 7: parser, workspace guard, cleanup và TCP probe.
- `prepush.ps1 -SkipAudit` chạy thành công từ thư mục ngoài repo. CI cũng chạy các bài kiểm tra mới, dùng Node 24 phù hợp toolchain hiện tại.

## Sửa lỗi thả file lên wheel nhưng không chuyển đổi

Hook chuột toàn cục nhận mouse-up trước khi Explorer hoàn tất OLE Drop. Đóng wheel ở mouse-up sẽ làm mất drop target, hoặc khiến frontend bỏ qua Drop vì wheel đã bị ẩn. Native hiện giữ phiên kéo khi con trỏ ở trong WebView, chỉ kết thúc qua OLE Drop/Leave hoặc hủy. Không dùng timeout để đoán thời điểm Drop.

Frontend chờ nhận diện đúng tập file trước khi xử lý Drop, chụp tọa độ tại thời điểm thả và chặn Drop lặp. Mỗi lượt kéo có revision riêng; hủy, mở Settings hoặc bắt đầu lượt mới sẽ vô hiệu hóa callback cũ, kể cả callback đang chờ IPC mở cửa sổ. Lỗi nhận diện được giữ trên wheel để người dùng thấy nguyên nhân.

Các bài hồi quy chạy qua component App thật với native events được mô phỏng: mouse-up trước Drop ở DPI 100%/150%/200%; Drop trong lúc đang nhận diện; Drop lặp; rời WebView trước/sau mouse-up; Escape; lỗi đọc file; hủy khi IPC đang mở cửa sổ và kéo lượt mới ngay sau đó. State machine native được kiểm tra ở cả hai thứ tự mouse-up/Drop.

`r.ps1 -Release` bật `tauri/custom-protocol` để nhúng frontend đã build vào executable, chạy độc lập với Vite.

## Giới hạn xác minh

Chưa thực hiện thao tác kéo từ Explorer trên desktop thật, kiểm thử tương tác trực tiếp với nhiều màn hình khác DPI, rút màn hình, OneDrive cloud placeholder, ổ dedup/mount hoặc game DirectX/Vulkan fullscreen. Code cho các nhánh này đã biên dịch và state machine được kiểm tra tự động; điều đó không thay thế kiểm tra trên thiết bị thực. Không khẳng định một hệ số tăng tốc vì chưa có benchmark trên tập dữ liệu đại diện.

Chưa chạy `cargo audit` vì môi trường không có cargo-audit. Không tạo installer, ký, cài đặt hoặc phát hành bản mới. Format không hỗ trợ loại metadata nguồn sẽ không thể giữ toàn bộ metadata khi chuyển đổi.

Tài liệu kỹ thuật tham chiếu: [image orientation](https://docs.rs/image/latest/image/metadata/enum.Orientation.html), [fast_image_resize](https://docs.rs/fast_image_resize/latest/fast_image_resize/), [FFmpeg filters](https://www.ffmpeg.org/ffmpeg-filters.html), [GetDpiForWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdpiforwindow).
