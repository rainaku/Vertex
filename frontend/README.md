# Vertex Frontend

Giao diện người dùng của Vertex, xây dựng bằng Svelte 5, TypeScript và Vite.

## Cấu trúc thư mục

- `src/App.svelte`: Quản lý sự kiện kéo thả chuột, tính toán vị trí vòng tròn và tương tác với backend Tauri.
- `src/lib/RadialWheel.svelte`: Vẽ menu tròn bằng SVG, xử lý hiệu ứng hover, chuyển trang và vòng sáng tiến độ chuyển đổi.
- `src/lib/types.ts`: Định nghĩa các kiểu dữ liệu cho target format, trạng thái chuyển đổi và sự kiện từ backend.

## Chạy riêng frontend

Để kiểm tra giao diện trên trình duyệt mà không cần khởi động toàn bộ ứng dụng desktop:

```bash
npm install
npm run dev
```
