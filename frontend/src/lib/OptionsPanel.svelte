<script lang="ts">
  import { Settings2, X, Sliders, Shield, FolderOpen } from 'lucide-svelte';
  import type { Options } from './types';

  export let options: Options = {
    quality: 85,
    dpi: 200,
    strip_metadata: true,
    collision_policy: 'rename_with_suffix',
  };
  export let visible: boolean = false;
  export let onClose: () => void = () => {};
</script>

{#if visible}
  <div
    class="modal-backdrop"
    role="presentation"
    on:click|self={onClose}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
  >
    <div class="modal-glass" role="dialog" aria-label="Tùy chọn chuyển đổi">
      <div class="modal-header">
        <div class="title-wrap">
          <Settings2 size={18} />
          <h3>Tùy chọn chuyển đổi</h3>
        </div>
        <button class="btn-close" on:click={onClose} aria-label="Đóng">
          <X size={16} />
        </button>
      </div>

      <div class="modal-body">
        <!-- Quality Slider -->
        <div class="setting-row">
          <div class="setting-label">
            <span class="label-title">Chất lượng ảnh ({options.quality}%)</span>
            <span class="label-desc">Áp dụng cho JPG, WEBP, AVIF</span>
          </div>
          <input
            type="range"
            min="10"
            max="100"
            step="5"
            bind:value={options.quality}
            class="slider"
          />
        </div>

        <!-- DPI Setting -->
        <div class="setting-row">
          <div class="setting-label">
            <span class="label-title">Độ phân giải DPI ({options.dpi})</span>
            <span class="label-desc">Áp dụng cho PDF sang ảnh</span>
          </div>
          <input
            type="number"
            min="72"
            max="600"
            step="50"
            bind:value={options.dpi}
            class="input-number"
          />
        </div>

        <!-- Strip Metadata Toggle -->
        <div class="setting-row inline">
          <div class="setting-label">
            <span class="label-title">Xóa siêu dữ liệu (EXIF / GPS)</span>
            <span class="label-desc">Bảo vệ quyền riêng tư</span>
          </div>
          <label class="switch">
            <input type="checkbox" bind:checked={options.strip_metadata} />
            <span class="switch-slider"></span>
          </label>
        </div>

        <!-- Collision Policy -->
        <div class="setting-row">
          <div class="setting-label">
            <span class="label-title">Quy tắc trùng tên</span>
            <span class="label-desc">Không bao giờ ghi đè mặc định</span>
          </div>
          <select bind:value={options.collision_policy} class="select">
            <option value="rename_with_suffix">Thêm hậu tố (1), (2)...</option>
            <option value="fail_if_exists">Báo lỗi nếu trùng</option>
            <option value="overwrite">Ghi đè file cũ</option>
          </select>
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn-primary" on:click={onClose}>Lưu & Đóng</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
    animation: fadeIn 0.2s ease-out;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .modal-glass {
    width: 360px;
    background: #000000;
    border: none;
    border-radius: 16px;
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.95);
    overflow: hidden;
    color: #ffffff;
    font-family: var(--font-family);
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid #27272a;
  }

  .title-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #ffffff;
  }

  .title-wrap h3 {
    font-size: 15px;
    font-weight: 700;
    font-family: var(--font-family);
  }

  .btn-close {
    background: transparent;
    border: none;
    color: #ffffff;
    cursor: pointer;
    padding: 4px;
    border-radius: 50%;
    transition: background 0.15s, color 0.15s;
  }

  .btn-close:hover {
    color: #000000;
    background: #ffffff;
  }

  .modal-body {
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .setting-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .setting-row.inline {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
  }

  .label-title {
    font-size: 13px;
    font-weight: 600;
    color: #ffffff;
    display: block;
    font-family: var(--font-family);
  }

  .label-desc {
    font-size: 11px;
    color: #a1a1aa;
    display: block;
    font-family: var(--font-family);
  }

  .slider {
    accent-color: #ffffff;
    cursor: pointer;
  }

  .input-number, .select {
    background: #111111;
    border: 1px solid #3f3f46;
    color: #ffffff;
    padding: 6px 10px;
    border-radius: 6px;
    font-size: 13px;
    font-family: var(--font-family);
    outline: none;
  }

  .input-number:focus, .select:focus {
    border-color: #ffffff;
  }

  .select option {
    background: #000000;
    color: #ffffff;
  }

  /* Switch */
  .switch {
    position: relative;
    display: inline-block;
    width: 40px;
    height: 22px;
  }

  .switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch-slider {
    position: absolute;
    cursor: pointer;
    inset: 0;
    background: #27272a;
    border: 1px solid #3f3f46;
    transition: 0.2s;
    border-radius: 9999px;
  }

  .switch-slider:before {
    position: absolute;
    content: "";
    height: 14px;
    width: 14px;
    left: 3px;
    bottom: 3px;
    background: #ffffff;
    transition: 0.2s;
    border-radius: 50%;
  }

  input:checked + .switch-slider {
    background: #ffffff;
    border-color: #ffffff;
  }

  input:checked + .switch-slider:before {
    background: #000000;
    transform: translateX(18px);
  }

  .modal-footer {
    padding: 12px 20px 16px;
    display: flex;
    justify-content: flex-end;
    border-top: 1px solid #27272a;
  }

  .btn-primary {
    background: #ffffff;
    border: none;
    color: #000000;
    font-family: var(--font-family);
    font-size: 13px;
    font-weight: 700;
    padding: 8px 18px;
    border-radius: 9999px;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .btn-primary:hover {
    background: #e4e4e7;
  }
</style>
