import { writable, derived, get } from 'svelte/store';
export type Language = 'vi' | 'en';
const key = 'vertex.language';
function initialLanguage(): Language {
  try { const saved = localStorage.getItem(key); if (saved === 'vi' || saved === 'en') return saved; } catch {}
  return navigator.language.toLowerCase().startsWith('vi') ? 'vi' : 'en';
}
export const language = writable<Language>(initialLanguage());
const vi = {
  "Cài đặt nâng cao": "Cài đặt nâng cao",
  "Đóng cài đặt": "Đóng cài đặt",
  "Cấu hình nhanh": "Cấu hình nhanh",
  "Cân bằng": "Cân bằng",
  "Nhẹ cho web": "Nhẹ cho web",
  "Chất lượng cao": "Chất lượng cao",
  "Ảnh & kích thước": "Ảnh & kích thước",
  "Chất lượng JPG / AVIF": "Chất lượng JPG / AVIF",
  "Rộng tối đa (px)": "Rộng tối đa (px)",
  "Cao tối đa (px)": "Cao tối đa (px)",
  "Giữ nguyên": "Giữ nguyên",
  "Độ trong suốt & mã hóa": "Độ trong suốt & mã hóa",
  "Màu nền JPG": "Màu nền JPG",
  "Ngưỡng alpha GIF": "Ngưỡng trong suốt GIF",
  "Tốc độ mã hóa AVIF": "Tốc độ mã hóa AVIF",
  "Lưu file": "Lưu file",
  "Thư mục đầu ra": "Thư mục đầu ra",
  "Cùng thư mục với file gốc": "Cùng thư mục với file gốc",
  "Đang chọn…": "Đang chọn…",
  "Chọn thư mục": "Chọn thư mục",
  "Dùng thư mục gốc": "Dùng thư mục gốc",
  "Khi trùng tên file": "Khi trùng tên file",
  "Thêm hậu tố (1), (2)…": "Thêm hậu tố (1), (2)…",
  "Báo lỗi, giữ file cũ": "Báo lỗi, giữ file cũ",
  "Ghi đè file cũ": "Ghi đè file cũ",
  "File đầu ra đã tồn tại sẽ bị thay thế.": "File đầu ra đã tồn tại sẽ bị thay thế.",
  "Mặc định": "Mặc định",
  "Hủy": "Hủy",
  "Lưu cài đặt": "Lưu cài đặt",
  "Không thể chọn thư mục. Hãy thử lại.": "Không thể chọn thư mục. Hãy thử lại.",
  "Không thể lưu cài đặt trên thiết bị này.": "Không thể lưu cài đặt trên thiết bị này.",
  "Áp dụng cho các lần chuyển đổi tiếp theo. Cài đặt được lưu trên thiết bị.": "Lưu trên máy, áp dụng cho những lần chuyển đổi sau.",
  "WEBP luôn được xuất lossless. PNG và TIFF giữ nguyên chất lượng mã hóa.": "WEBP, PNG và TIFF dùng nén không mất dữ liệu.",
  "Ảnh giữ tỷ lệ, chỉ thu nhỏ. Để trống để giữ kích thước gốc. Giới hạn cũng được gửi tới bộ chuyển đổi video.": "Ảnh chỉ thu nhỏ và giữ tỷ lệ. Để trống để giữ kích thước gốc. Cũng áp dụng khi chuyển đổi video.",
  "JPG không có nền trong suốt. Các định dạng hỗ trợ alpha vẫn giữ nền trong suốt.": "JPG dùng màu nền này. Định dạng hỗ trợ trong suốt vẫn giữ nền trong suốt.",
  "Pixel dưới ngưỡng sẽ trong suốt. GIF chỉ hỗ trợ trong suốt hoàn toàn hoặc đục hoàn toàn.": "Điểm ảnh dưới ngưỡng sẽ trong suốt. GIF không giữ được độ trong suốt một phần.",
  "1: chậm, nén hiệu quả hơn · 10: nhanh hơn.": "1: chậm, nén hiệu quả hơn · 10: nhanh hơn.",
  "Mở cài đặt nâng cao": "Mở cài đặt nâng cao",
  "Chuyển đổi hoàn tất": "Chuyển đổi hoàn tất",
  "Mở thư mục chứa file": "Mở thư mục chứa file",
  "Đóng thông báo": "Đóng thông báo",
  "Mở thư mục": "Mở thư mục",
  "Xong": "Xong",
  "Trang": "Trang",
  "Trang tiếp": "Trang tiếp",
  "Định dạng": "Định dạng",
  "Ảnh": "Ảnh",
  "Âm thanh": "Âm thanh",
  "Video": "Video",
  "Tài liệu": "Tài liệu",
  "Tệp": "Tệp",
  "+ thả để chuyển đổi": "+ thả để chuyển đổi",
  "Tệp được hỗ trợ": "Tệp được hỗ trợ",
  "Ngôn ngữ": "Ngôn ngữ",
  "Không thể đổi ngôn ngữ. Hãy thử lại.": "Không thể đổi ngôn ngữ. Hãy thử lại.",
  "Cần cài FFmpeg để dùng định dạng này.": "Cần cài FFmpeg để dùng định dạng này.",
  "Định dạng này chưa dùng được.": "Định dạng này chưa dùng được.",
  "Vertex – Chuyển đổi tệp": "Vertex – Chuyển đổi tệp"
};
const en: Record<keyof typeof vi, string> = {
  "Cài đặt nâng cao": "Advanced settings",
  "Đóng cài đặt": "Close settings",
  "Cấu hình nhanh": "Presets",
  "Cân bằng": "Balanced",
  "Nhẹ cho web": "Smaller files",
  "Chất lượng cao": "High quality",
  "Ảnh & kích thước": "Image and size",
  "Chất lượng JPG / AVIF": "JPG / AVIF quality",
  "Rộng tối đa (px)": "Max width (px)",
  "Cao tối đa (px)": "Max height (px)",
  "Giữ nguyên": "Original size",
  "Độ trong suốt & mã hóa": "Transparency and encoding",
  "Màu nền JPG": "JPG background",
  "Ngưỡng alpha GIF": "GIF transparency threshold",
  "Tốc độ mã hóa AVIF": "AVIF encoding speed",
  "Lưu file": "Output",
  "Thư mục đầu ra": "Output folder",
  "Cùng thư mục với file gốc": "Same folder as source",
  "Đang chọn…": "Choosing…",
  "Chọn thư mục": "Choose folder",
  "Dùng thư mục gốc": "Use source folder",
  "Khi trùng tên file": "If a file already exists",
  "Thêm hậu tố (1), (2)…": "Add (1), (2)… to the name",
  "Báo lỗi, giữ file cũ": "Stop and keep the existing file",
  "Ghi đè file cũ": "Replace the existing file",
  "File đầu ra đã tồn tại sẽ bị thay thế.": "Existing output files will be replaced.",
  "Mặc định": "Reset",
  "Hủy": "Cancel",
  "Lưu cài đặt": "Save",
  "Không thể chọn thư mục. Hãy thử lại.": "Couldn’t open the folder picker. Try again.",
  "Không thể lưu cài đặt trên thiết bị này.": "Couldn’t save settings on this device.",
  "Áp dụng cho các lần chuyển đổi tiếp theo. Cài đặt được lưu trên thiết bị.": "Saved on this device for future conversions.",
  "WEBP luôn được xuất lossless. PNG và TIFF giữ nguyên chất lượng mã hóa.": "WEBP, PNG and TIFF use lossless encoding.",
  "Ảnh giữ tỷ lệ, chỉ thu nhỏ. Để trống để giữ kích thước gốc. Giới hạn cũng được gửi tới bộ chuyển đổi video.": "Images keep their proportions and only scale down. Leave blank for original size. Also applies to video conversion.",
  "JPG không có nền trong suốt. Các định dạng hỗ trợ alpha vẫn giữ nền trong suốt.": "JPG uses this background. Formats that support transparency keep it.",
  "Pixel dưới ngưỡng sẽ trong suốt. GIF chỉ hỗ trợ trong suốt hoàn toàn hoặc đục hoàn toàn.": "Pixels below this threshold become transparent. GIF cannot keep partial transparency.",
  "1: chậm, nén hiệu quả hơn · 10: nhanh hơn.": "1: slower, better compression · 10: faster.",
  "Mở cài đặt nâng cao": "Open advanced settings",
  "Chuyển đổi hoàn tất": "Conversion complete",
  "Mở thư mục chứa file": "Show in folder",
  "Đóng thông báo": "Dismiss notification",
  "Mở thư mục": "Show in folder",
  "Xong": "Done",
  "Trang": "Page",
  "Trang tiếp": "Next page",
  "Định dạng": "Format",
  "Ảnh": "Image",
  "Âm thanh": "Audio",
  "Video": "Video",
  "Tài liệu": "Document",
  "Tệp": "Files",
  "+ thả để chuyển đổi": "+ drop to convert",
  "Tệp được hỗ trợ": "Supported files",
  "Ngôn ngữ": "Language",
  "Không thể đổi ngôn ngữ. Hãy thử lại.": "Couldn’t change language. Try again.",
  "Cần cài FFmpeg để dùng định dạng này.": "Install FFmpeg to use this format.",
  "Định dạng này chưa dùng được.": "This format is unavailable.",
  "Vertex – Chuyển đổi tệp": "Vertex – File converter"
};
export type MessageKey = keyof typeof vi;
export const t = derived(language, lang => (key: MessageKey) => (lang === 'vi' ? vi : en)[key]);
export function translate(key: MessageKey) { return get(t)(key); }
export async function syncNativeLanguage(lang: Language) {
  if ('__TAURI_INTERNALS__' in window) {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('set_app_language', { language: lang });
  }
}
export async function setLanguage(lang: Language) {
  await syncNativeLanguage(lang);
  localStorage.setItem(key, lang);
  language.set(lang);
}
language.subscribe(lang => {
  document.documentElement.lang = lang;
  document.title = (lang === 'vi' ? vi : en)['Vertex – Chuyển đổi tệp'];
});
