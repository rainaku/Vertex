# Chính sách Quyền riêng tư — Vertex

**Ngày có hiệu lực:** 01 tháng 10 năm 2026  
**Người phát triển:** rainaku  
**Dự án:** https://github.com/rainaku/Vertex  
**Liên hệ:** [github.com/rainaku/Vertex/issues](https://github.com/rainaku/Vertex/issues)  

---

## 1. Tổng quan

Vertex là công cụ chuyển đổi tệp đa phương tiện dạng bánh xe hướng tâm miễn phí và mã nguồn mở dành cho Windows. Chính sách này giải thích chi tiết về quyền truy cập dữ liệu trên máy tính, quy trình xử lý và các kết nối mạng của ứng dụng.

**Nguyên tắc cốt lõi:** Vertex hoạt động 100% cục bộ (Local-First). Ứng dụng không thu thập thông tin người dùng, không có quảng cáo, không gắn mã theo dõi (tracking), không phân tích hành vi (analytics) và không vận hành bất kỳ máy chủ thu thập dữ liệu nào. Mọi tệp tin bạn kéo thả vào ứng dụng đều được xử lý trực tiếp trên máy của bạn và không bao giờ rời khỏi thiết bị.

---

## 2. Bảng tóm tắt hoạt động

| Tính năng | Dữ liệu truy cập | Rời khỏi thiết bị? | Lưu trên ổ cứng? |
|---|---|---|---|
| **Chuyển đổi tệp** | Đường dẫn và nội dung tệp được kéo thả | **Không** (Xử lý 100% trên CPU/GPU qua lõi Rust và FFmpeg) | Có, chỉ lưu dưới dạng tệp kết quả tại thư mục bạn chọn |
| **Cử chỉ Shift + Kéo thả** | Vị trí con trỏ chuột và trạng thái phím Shift | **Không** | Không (Chỉ xử lý tức thì trong RAM) |
| **Cài đặt & Tùy chọn** | Ngôn ngữ giao diện, chất lượng ảnh, DPI, nén video | **Không** | Có (Lưu cục bộ trong bộ nhớ WebView2 của máy) |
| **Kiểm tra bản cập nhật** | Thẻ phiên bản hiện tại gửi qua tiêu đề HTTP chuẩn | **Có** — GitHub Releases API (`api.github.com` / `github.com`) | Lưu bộ cài đặt tạm thời trong `%TEMP%` khi bạn chọn cập nhật |
| **Liên kết ngoài** | Mở liên kết trình duyệt (GitHub, PayPal, Trang chủ) | **Có** — Chỉ chuyển hướng tới trang bạn bấm chọn | Lưu trong lịch sử duyệt web thông thường của bạn |

---

## 3. Dữ liệu được xử lý trên máy tính

### 3.1 Xử lý tệp
Khi bạn giữ phím `Shift` và kéo tệp qua bánh xe chuyển đổi:
- Ứng dụng đọc đường dẫn tệp để nhận diện loại định dạng qua chữ ký tệp (magic bytes) bằng thư viện `infer` nội bộ.
- Khi thả vào một cánh định dạng, luồng dữ liệu tệp được chuyển trực tiếp vào bộ mã hóa (Rust image codecs hoặc tiến trình `ffmpeg.exe` cục bộ).
- Tệp sau khi chuyển đổi được lưu trực tiếp vào thư mục nguồn (hoặc thư mục bạn đã chỉ định).
- Không có bất kỳ phần dữ liệu nào bị truyền qua mạng.

### 3.2 Phím tắt và Chuột
- Vertex theo dõi phím `Shift` và nút chuột trái qua hàm Windows API (`SetWindowsHookExW` và `GetAsyncKeyState`).
- Các sự kiện này chỉ phục vụ mục đích duy nhất: phát hiện thao tác kéo tệp để hiển thị giao diện bánh xe ngay tại vị trí trỏ chuột.
- Ứng dụng không ghi lại hay lưu trữ nội dung gõ phím của bạn.

### 3.3 Thiết lập người dùng
- Các tùy chọn như ngôn ngữ (`vi` / `en`), DPI, chất lượng hình ảnh và nén video được lưu tại bộ nhớ cục bộ của trình duyệt WebView2 trên máy tính.
- Các thiết lập này không đồng bộ lên mạng.

---

## 4. Các kết nối mạng duy nhất

Ứng dụng chỉ tạo kết nối mạng ra ngoài trong các trường hợp sau:

1. **Kiểm tra cập nhật tự động / thủ công:**
   - **Địa chỉ:** `https://github.com/rainaku/Vertex/releases/latest/download/latest.json` và `https://api.github.com/repos/rainaku/Vertex/releases/latest`.
   - **Dữ liệu gửi đi:** Yêu cầu HTTP GET tiêu chuẩn. Không gửi mã định danh máy tính, thông tin phần cứng hay dữ liệu cá nhân.
   - **Tần suất:** 15 giây sau khi mở ứng dụng, lặp lại mỗi 6 tiếng, hoặc khi bạn bấm nút "Kiểm tra cập nhật" trong Cài đặt.
2. **Tải bộ cài đặt mới:**
   - Khi bạn đồng ý nâng cấp, file bộ cài sẽ được tải trực tiếp từ GitHub Releases vào thư mục `%TEMP%` và kiểm tra chữ ký số trước khi thực thi.
3. **Mở liên kết web:**
   - Khi bạn bấm vào biểu tượng GitHub, Donate hoặc mạng xã hội.

---

## 5. Quyền của bạn

Vì Vertex không lưu trữ bất kỳ thông tin cá nhân nào trên máy chủ từ xa, bạn nắm toàn quyền kiểm soát dữ liệu trên máy tính của mình. Để xóa toàn bộ thiết lập ứng dụng, bạn chỉ cần gỡ cài đặt và xóa thư mục `%LOCALAPPDATA%\com.vertex.converter`.

---

## 6. Liên hệ

Mọi thắc mắc về quyền riêng tư, vui lòng mở phản hồi tại:  
https://github.com/rainaku/Vertex/issues
