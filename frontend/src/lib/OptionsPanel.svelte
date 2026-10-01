<script lang="ts">
  import { t, language, setLanguage } from './i18n';
  import type { Options } from './types';
  import { defaultOptions, normalizeOptions, SETTINGS_KEY } from './settings';
  import { isTauri } from './tauri-bridge';
  import ElasticSlider from './ElasticSlider.svelte';

  export let options: Options;
  export let onSave: (value: Options) => void;
  export let onClose: () => void;

  let draft = normalizeOptions(structuredClone(options));
  let error = '';
  let pickingFolder = false;
  let activeTab: 'general' | 'image' | 'advanced' = 'general';

  $: backgroundHex = '#' + (draft.jpeg_background ?? [255, 255, 255])
    .map(n => n.toString(16).padStart(2, '0'))
    .join('');

  $: activePreset =
    draft.quality === 80 && draft.max_width === 1920 && draft.max_height === 1920 && draft.avif_speed === 8
      ? 'web'
      : draft.quality === 100 && !draft.max_width && !draft.max_height && draft.avif_speed === 3
      ? 'quality'
      : draft.quality === 85 && !draft.max_width && !draft.max_height && draft.avif_speed === 6
      ? 'balanced'
      : null;

  function setBackground(event: Event) {
    const hex = (event.target as HTMLInputElement).value;
    draft.jpeg_background = [1, 3, 5].map(offset =>
      parseInt(hex.slice(offset, offset + 2), 16)
    ) as [number, number, number];
  }

  function preset(name: 'balanced' | 'web' | 'quality') {
    const values = name === 'web'
      ? { quality: 80, max_width: 1920, max_height: 1920, avif_speed: 8 }
      : name === 'quality'
      ? { quality: 100, max_width: undefined, max_height: undefined, avif_speed: 3 }
      : { quality: 85, max_width: undefined, max_height: undefined, avif_speed: 6 };
    draft = { ...draft, ...values };
  }

  async function chooseFolder() {
    pickingFolder = true;
    error = '';
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false, title: $t("Thư mục đầu ra") });
      if (typeof selected === 'string') draft.output_dir = selected;
    } catch {
      error = $t("Không thể chọn thư mục. Hãy thử lại.");
    } finally {
      pickingFolder = false;
    }
  }

  function save() {
    const value = normalizeOptions(draft);
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(value));
      onSave(value);
    } catch {
      error = $t("Không thể lưu cài đặt trên thiết bị này.");
    }
  }

  function resetToDefault() {
    draft = defaultOptions();
  }

  function focusDialog(node: HTMLElement) {
    const previous = document.activeElement as HTMLElement | null;
    node.focus();
    function keydown(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        if (!pickingFolder) onClose();
      }
      if (event.key !== 'Tab') return;
      const elements = Array.from(node.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input:not(:disabled), select:not(:disabled)'
      ));
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (event.shiftKey && (document.activeElement === first || document.activeElement === node)) {
        event.preventDefault(); last?.focus();
      } else if (!event.shiftKey && (document.activeElement === last || document.activeElement === node)) {
        event.preventDefault(); first?.focus();
      }
    }
    node.addEventListener('keydown', keydown);
    return { destroy() { node.removeEventListener('keydown', keydown); previous?.focus(); } };
  }
</script>

<div class="backdrop" role="presentation" on:click|self={onClose}>
  <div class="panel" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1" use:focusDialog>
    <!-- Header: Sparkle + Title + Close Button -->
    <header class="header">
      <div class="header-left">
        <svg class="sparkle-icon" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 0L14.7 9.3L24 12L14.7 14.7L12 24L9.3 14.7L0 12L9.3 9.3L12 0Z"/>
        </svg>
        <h2 id="settings-title">{$t("Cài đặt")}</h2>
      </div>
      <button type="button" class="btn-close" on:click={onClose} disabled={pickingFolder} aria-label={$t("Đóng cài đặt")}>
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
          <line x1="1" y1="1" x2="9" y2="9" />
          <line x1="9" y1="1" x2="1" y2="9" />
        </svg>
      </button>
    </header>

    <!-- Segmented Navigation Tabs: V-Notch Style -->
    <nav class="tabs-bar" aria-label="Settings Categories">
      <button
        type="button"
        class="tab-pill"
        class:active={activeTab === 'general'}
        on:click={() => activeTab = 'general'}
      >
        {$t("Chung")}
      </button>
      <button
        type="button"
        class="tab-pill"
        class:active={activeTab === 'image'}
        on:click={() => activeTab = 'image'}
      >
        {$t("Hình ảnh")}
      </button>
      <button
        type="button"
        class="tab-pill"
        class:active={activeTab === 'advanced'}
        on:click={() => activeTab = 'advanced'}
      >
        {$t("Nâng cao")}
      </button>
    </nav>

    <!-- Main Content Form -->
    <form on:submit|preventDefault={save} class="form-container">
      <div class="content-scroll">
        <!-- TAB 1: GENERAL -->
        {#if activeTab === 'general'}
          <div class="tab-pane">
            <!-- Language -->
            <div class="setting-item">
              <span class="setting-title">{$t("Ngôn ngữ")}</span>
              <span class="setting-desc">{$language === 'vi' ? 'Ngôn ngữ hiển thị toàn bộ giao diện.' : 'Interface display language.'}</span>
              <div class="segmented-control">
                <button
                  type="button"
                  class="seg-item"
                  class:active={$language === 'vi'}
                  on:click={() => setLanguage('vi')}
                >
                  Tiếng Việt
                </button>
                <button
                  type="button"
                  class="seg-item"
                  class:active={$language === 'en'}
                  on:click={() => setLanguage('en')}
                >
                  English
                </button>
              </div>
            </div>

            <!-- Presets -->
            <div class="setting-item">
              <span class="setting-title">{$t("Cấu hình nhanh")}</span>
              <span class="setting-desc">{$language === 'vi' ? 'Áp dụng tức thì cho các chuyển đổi tiếp theo.' : 'Saved on this device for future conversions.'}</span>
              <div class="chips-row">
                <button
                  type="button"
                  class="chip-btn"
                  class:active={activePreset === 'balanced'}
                  on:click={() => preset('balanced')}
                >
                  {$t("Cân bằng")}
                </button>
                <button
                  type="button"
                  class="chip-btn"
                  class:active={activePreset === 'web'}
                  on:click={() => preset('web')}
                >
                  {$t("Nhẹ cho web")}
                </button>
                <button
                  type="button"
                  class="chip-btn"
                  class:active={activePreset === 'quality'}
                  on:click={() => preset('quality')}
                >
                  {$t("Chất lượng cao")}
                </button>
              </div>
            </div>

            <!-- Output Folder -->
            <div class="setting-item">
              <span class="setting-title">{$t("Thư mục đầu ra")}</span>
              <span class="setting-desc">{$t("Cùng thư mục với file gốc")}</span>
              <div class="folder-strip">
                <div class="folder-path" title={draft.output_dir || $t("Cùng thư mục với file gốc")}>
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="folder-icon">
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                  </svg>
                  <span class="path-text">{draft.output_dir || $t("Cùng thư mục với file gốc")}</span>
                </div>
                <div class="folder-btns">
                  <button type="button" class="btn-browse" on:click={chooseFolder} disabled={!isTauri || pickingFolder}>
                    {pickingFolder ? '…' : $t("Duyệt…")}
                  </button>
                  {#if draft.output_dir}
                    <button type="button" class="btn-clear" on:click={() => draft.output_dir = undefined} title={$t("Dùng thư mục gốc")}>
                      <svg width="9" height="9" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
                        <line x1="1" y1="1" x2="9" y2="9" />
                        <line x1="9" y1="1" x2="1" y2="9" />
                      </svg>
                    </button>
                  {/if}
                </div>
              </div>
            </div>

            <!-- Conflict Policy -->
            <div class="setting-item">
              <span class="setting-title">{$t("Khi trùng tên file")}</span>
              <span class="setting-desc">{$language === 'vi' ? 'Quy tắc xử lý nếu file đích đã tồn tại trong thư mục.' : 'Behavior when an output file already exists.'}</span>
              <div class="segmented-control full">
                <button
                  type="button"
                  class="seg-item"
                  class:active={draft.collision_policy === 'rename_with_suffix'}
                  on:click={() => draft.collision_policy = 'rename_with_suffix'}
                >
                  {$language === 'vi' ? 'Đổi tên (+1)' : 'Rename'}
                </button>
                <button
                  type="button"
                  class="seg-item"
                  class:active={draft.collision_policy === 'fail_if_exists'}
                  on:click={() => draft.collision_policy = 'fail_if_exists'}
                >
                  {$language === 'vi' ? 'Dừng lại' : 'Stop'}
                </button>
                <button
                  type="button"
                  class="seg-item danger"
                  class:active={draft.collision_policy === 'overwrite'}
                  on:click={() => draft.collision_policy = 'overwrite'}
                >
                  {$language === 'vi' ? 'Ghi đè' : 'Overwrite'}
                </button>
              </div>
              {#if draft.collision_policy === 'overwrite'}
                <div class="warn-line">
                  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/>
                    <line x1="12" y1="9" x2="12" y2="13"/>
                    <line x1="12" y1="17" x2="12.01" y2="17"/>
                  </svg>
                  <span>{$t("File đầu ra đã tồn tại sẽ bị thay thế.")}</span>
                </div>
              {/if}
            </div>
          </div>
        {/if}

        <!-- TAB 2: IMAGE -->
        {#if activeTab === 'image'}
          <div class="tab-pane">
            <!-- Quality: ELASTIC SLIDER -->
            <div class="setting-item">
              <span class="setting-title">{$t("Chất lượng JPG / AVIF")}</span>
              <span class="setting-desc">{$t("WEBP luôn được xuất lossless. PNG và TIFF giữ nguyên chất lượng mã hóa.")}</span>
              <ElasticSlider
                bind:value={draft.quality}
                min={1}
                max={100}
                step={1}
                unit="%"
                ticks={16}
              />
            </div>

            <!-- PDF DPI: ELASTIC SLIDER -->
            <div class="setting-item">
              <span class="setting-title">{$t("Độ phân giải PDF (DPI)")}</span>
              <span class="setting-desc">{$language === 'vi' ? 'Mật độ điểm ảnh khi xuất tài liệu PDF ra ảnh.' : 'Rendering pixel density for PDF pages.'}</span>
              <ElasticSlider
                bind:value={draft.dpi}
                min={72}
                max={600}
                step={10}
                unit="DPI"
                ticks={14}
              />
            </div>

            <!-- Strip EXIF Metadata -->
            <div class="setting-item row-split">
              <div class="setting-text">
                <span class="setting-title">{$t("Xóa siêu dữ liệu (EXIF)")}</span>
                <span class="setting-desc">{$t("Bảo vệ vị trí & quyền riêng tư")}</span>
              </div>
              <label class="switch">
                <input type="checkbox" bind:checked={draft.strip_metadata} />
                <span class="switch-knob"></span>
              </label>
            </div>

            <!-- Max Dimensions -->
            <div class="setting-item">
              <span class="setting-title">{$t("Kích thước tối đa (px)")}</span>
              <span class="setting-desc">{$t("Ảnh giữ tỷ lệ, chỉ thu nhỏ. Để trống để giữ kích thước gốc. Giới hạn cũng được gửi tới bộ chuyển đổi video.")}</span>
              <div class="dual-inputs">
                <div class="input-box">
                  <span class="input-tag">{$t("Rộng tối đa (px)")}</span>
                  <input
                    type="number"
                    min="1"
                    max="32768"
                    step="1"
                    placeholder={$t("Giữ nguyên")}
                    bind:value={draft.max_width}
                    class="field-input"
                  />
                </div>
                <div class="input-box">
                  <span class="input-tag">{$t("Cao tối đa (px)")}</span>
                  <input
                    type="number"
                    min="1"
                    max="32768"
                    step="1"
                    placeholder={$t("Giữ nguyên")}
                    bind:value={draft.max_height}
                    class="field-input"
                  />
                </div>
              </div>
            </div>
          </div>
        {/if}

        <!-- TAB 3: ADVANCED -->
        {#if activeTab === 'advanced'}
          <div class="tab-pane">
            <!-- GIF Alpha Threshold: ELASTIC SLIDER -->
            <div class="setting-item">
              <span class="setting-title">{$t("Ngưỡng alpha GIF")}</span>
              <span class="setting-desc">{$t("Pixel dưới ngưỡng sẽ trong suốt. GIF chỉ hỗ trợ trong suốt hoàn toàn hoặc đục hoàn toàn.")}</span>
              <ElasticSlider
                bind:value={draft.gif_alpha_threshold}
                min={1}
                max={255}
                step={1}
                unit="/ 255"
                ticks={16}
              />
            </div>

            <!-- AVIF Speed: ELASTIC SLIDER -->
            <div class="setting-item">
              <span class="setting-title">{$t("Tốc độ mã hóa AVIF")}</span>
              <span class="setting-desc">{$t("1: chậm, nén hiệu quả hơn · 10: nhanh hơn.")}</span>
              <ElasticSlider
                bind:value={draft.avif_speed}
                min={1}
                max={10}
                step={1}
                unit="/ 10"
                ticks={10}
              />
            </div>

            <!-- Video CRF Quality: ELASTIC SLIDER -->
            <div class="setting-item">
              <span class="setting-title">{$t("Chất lượng video (CRF)")}</span>
              <span class="setting-desc">{$t("Số thấp giữ nhiều chi tiết hơn, số cao thường cho file nhỏ hơn.")}</span>
              <ElasticSlider
                bind:value={draft.video_crf}
                min={18}
                max={35}
                step={1}
                unit="CRF"
                ticks={12}
              />
            </div>

            <!-- Video Codec -->
            <div class="setting-item">
              <span class="setting-title">{$t("Bộ mã hóa")}</span>
              <span class="setting-desc">{draft.video_codec === 'h265' ? $t("H.265: nén hiệu quả, cần thiết bị hỗ trợ") : $t("H.264: dễ phát trên nhiều thiết bị")}</span>
              <div class="segmented-control full">
                <button
                  type="button"
                  class="seg-item"
                  class:active={draft.video_codec === 'h264'}
                  on:click={() => draft.video_codec = 'h264'}
                >
                  H.264
                </button>
                <button
                  type="button"
                  class="seg-item"
                  class:active={draft.video_codec === 'h265'}
                  on:click={() => draft.video_codec = 'h265'}
                >
                  H.265
                </button>
              </div>
            </div>

            <!-- JPG Background Color -->
            <div class="setting-item row-split">
              <div class="setting-text">
                <span class="setting-title">{$t("Màu nền JPG")}</span>
                <span class="setting-desc">{$t("JPG không có nền trong suốt. Các định dạng hỗ trợ alpha vẫn giữ nền trong suốt.")}</span>
              </div>
              <label class="color-swatch-pill">
                <span class="swatch-circle" style="background-color: {backgroundHex};"></span>
                <span class="swatch-text">{backgroundHex.toUpperCase()}</span>
                <input type="color" value={backgroundHex} on:input={setBackground} class="color-native" />
              </label>
            </div>
          </div>
        {/if}

        {#if error}
          <div class="error-msg" role="alert">{error}</div>
        {/if}
      </div>

      <!-- Footer: V-Notch Style (Đặt lại | Áp dụng / Hủy | Lưu) -->
      <footer class="footer">
        <button type="button" class="btn-reset" on:click={resetToDefault}>
          {$language === 'vi' ? 'Đặt lại' : $t("Mặc định")}
        </button>
        <div class="footer-right">
          <button type="button" class="btn-cancel" on:click={onClose} disabled={pickingFolder}>
            {$t("Hủy")}
          </button>
          <button type="submit" class="btn-save" disabled={pickingFolder}>
            {$language === 'vi' ? 'Lưu' : $t("Lưu cài đặt")}
          </button>
        </div>
      </footer>
    </form>
  </div>
</div>

<style>
  /* ─── Backdrop Overlay ─────────────────────────────────────────────────────── */
  .backdrop {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 600;
    background: rgba(0, 0, 0, 0.72);
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    animation: modalFadeIn 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes modalFadeIn {
    from { opacity: 0; transform: scale(0.96); }
    to { opacity: 1; transform: scale(1); }
  }

  /* ─── Panel: V-Notch Obsidian Design (Fits 480x480) ────────────────────────── */
  .panel {
    width: 414px;
    height: 432px;
    display: flex;
    flex-direction: column;
    background: #090a0d;
    color: #f4f4f5;
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 18px;
    box-shadow:
      0 32px 80px -10px rgba(0, 0, 0, 0.98),
      0 0 0 1px rgba(255, 255, 255, 0.04),
      inset 0 1px 0 rgba(255, 255, 255, 0.1);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    overflow: hidden;
  }

  /* ─── Header: Sparkle + Title ──────────────────────────────────────────────── */
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 18px 10px;
    flex-shrink: 0;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .sparkle-icon {
    color: #ffffff;
    flex-shrink: 0;
  }

  h2 {
    font-size: 17px;
    font-weight: 700;
    color: #ffffff;
    margin: 0;
    letter-spacing: -0.02em;
    line-height: 1.2;
  }

  .btn-close {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    background: rgba(255, 255, 255, 0.05);
    color: #a1a1aa;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 7px;
    cursor: pointer;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .btn-close:hover {
    background: rgba(255, 255, 255, 0.15);
    color: #ffffff;
    transform: scale(1.05);
  }

  /* ─── Segmented Tabs Bar ───────────────────────────────────────────────────── */
  .tabs-bar {
    display: flex;
    gap: 4px;
    padding: 3px;
    margin: 0 18px 10px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 9px;
    flex-shrink: 0;
  }

  .tab-pill {
    flex: 1;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 500;
    color: #71717a;
    background: transparent;
    border: 0;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .tab-pill:hover {
    color: #d4d4d8;
  }

  .tab-pill.active {
    background: #ffffff;
    color: #09090b;
    font-weight: 600;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
  }

  /* ─── Scrollable Form Content ──────────────────────────────────────────────── */
  .form-container {
    min-height: 0;
    display: flex;
    flex-direction: column;
    flex: 1;
  }

  .content-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 18px 8px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.12) transparent;
  }

  .content-scroll::-webkit-scrollbar {
    width: 3px;
  }
  .content-scroll::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.12);
    border-radius: 999px;
  }

  .tab-pane {
    display: flex;
    flex-direction: column;
    gap: 13px;
    animation: tabSlideFade 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes tabSlideFade {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  /* ─── Setting Items (V-Notch Style) ────────────────────────────────────────── */
  .setting-item {
    display: flex;
    flex-direction: column;
  }

  .setting-item.row-split {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .setting-text {
    display: flex;
    flex-direction: column;
  }

  .setting-title {
    font-size: 13px;
    font-weight: 600;
    color: #ffffff;
    margin-bottom: 2px;
    line-height: 1.3;
  }

  .setting-desc {
    font-size: 11px;
    color: #71717a;
    line-height: 1.4;
    margin-bottom: 8px;
  }

  .setting-item.row-split .setting-desc {
    margin-bottom: 0;
  }

  /* ─── Segmented Pills Control ──────────────────────────────────────────────── */
  .segmented-control {
    display: flex;
    gap: 3px;
    background: rgba(255, 255, 255, 0.04);
    padding: 3px;
    border-radius: 9px;
    border: 1px solid rgba(255, 255, 255, 0.07);
    flex-shrink: 0;
  }

  .segmented-control.full {
    width: 100%;
  }

  .seg-item {
    flex: 1;
    height: 28px;
    padding: 0 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 500;
    color: #a1a1aa;
    background: transparent;
    border: 0;
    border-radius: 6px;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .seg-item:hover {
    color: #ffffff;
  }

  .seg-item.active {
    background: #ffffff;
    color: #000000;
    font-weight: 600;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
  }

  .seg-item.danger.active {
    background: #ef4444;
    color: #ffffff;
  }

  /* ─── Presets Chips Row ────────────────────────────────────────────────────── */
  .chips-row {
    display: flex;
    gap: 6px;
  }

  .chip-btn {
    flex: 1;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 500;
    color: #d4d4d8;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .chip-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  .chip-btn.active {
    background: rgba(255, 255, 255, 0.15);
    border-color: rgba(255, 255, 255, 0.22);
    color: #ffffff;
    font-weight: 600;
  }

  /* ─── Folder Selector Strip ────────────────────────────────────────────────── */
  .folder-strip {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .folder-path {
    flex: 1;
    min-width: 0;
    height: 34px;
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 10px;
    padding: 0 12px;
    font-size: 11px;
    color: #d4d4d8;
  }

  .folder-icon {
    color: #71717a;
    flex-shrink: 0;
  }

  .path-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .folder-btns {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }

  .btn-browse {
    height: 34px;
    padding: 0 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 500;
    color: #f4f4f5;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 9px;
    cursor: pointer;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .btn-browse:hover {
    background: rgba(255, 255, 255, 0.16);
  }

  .btn-clear {
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    color: #71717a;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 9px;
    cursor: pointer;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .btn-clear:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
  }

  .warn-line {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    color: #fb923c;
    margin-top: 4px;
  }

  /* ─── Toggle Switch ────────────────────────────────────────────────────────── */
  .switch {
    position: relative;
    display: inline-block;
    width: 36px;
    height: 20px;
    flex-shrink: 0;
    cursor: pointer;
  }

  .switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch-knob {
    position: absolute;
    inset: 0;
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 999px;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .switch-knob:before {
    position: absolute;
    content: "";
    height: 14px;
    width: 14px;
    left: 2px;
    bottom: 2px;
    background: #ffffff;
    border-radius: 50%;
    transition: transform 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
  }

  .switch input:checked + .switch-knob {
    background: #ffffff;
  }

  .switch input:checked + .switch-knob:before {
    transform: translateX(16px);
    background: #000000;
  }

  /* ─── Dual Dimension Inputs ────────────────────────────────────────────────── */
  .dual-inputs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .input-box {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .input-tag {
    font-size: 9.5px;
    color: #71717a;
  }

  .field-input {
    width: 100%;
    height: 32px;
    font-size: 11px;
    color: #ffffff;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 8px;
    padding: 0 10px;
    outline: none;
    font-family: inherit;
    transition: border-color 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .field-input:focus {
    border-color: rgba(255, 255, 255, 0.25);
  }

  /* ─── Color Swatch Pill ────────────────────────────────────────────────────── */
  .color-swatch-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 999px;
    padding: 0 10px 0 5px;
    cursor: pointer;
    position: relative;
    transition: background 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .color-swatch-pill:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .swatch-circle {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.35);
  }

  .swatch-text {
    font-size: 10.5px;
    font-weight: 600;
    color: #ffffff;
    font-variant-numeric: tabular-nums;
  }

  .color-native {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
    pointer-events: none;
  }

  .error-msg {
    font-size: 10.5px;
    color: #f87171;
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.2);
    border-radius: 8px;
    padding: 8px 12px;
    margin-top: 4px;
  }

  /* ─── Footer: V-Notch Style ────────────────────────────────────────────────── */
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 18px 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
    background: #090a0d;
  }

  .btn-reset {
    font-size: 12px;
    font-weight: 500;
    color: #71717a;
    background: transparent;
    border: 0;
    cursor: pointer;
    padding: 6px 4px;
    border-radius: 6px;
    transition: color 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .btn-reset:hover {
    color: #e4e4e7;
  }

  .footer-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .btn-cancel {
    height: 32px;
    padding: 0 16px;
    font-size: 12px;
    font-weight: 600;
    color: #ffffff;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    cursor: pointer;
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .btn-cancel:hover {
    background: rgba(255, 255, 255, 0.14);
  }

  .btn-save {
    height: 32px;
    padding: 0 20px;
    font-size: 12px;
    font-weight: 700;
    color: #000000;
    background: #ffffff;
    border: 0;
    border-radius: 10px;
    cursor: pointer;
    box-shadow: 0 2px 10px rgba(255, 255, 255, 0.18);
    transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .btn-save:hover {
    background: #e4e4e7;
    transform: translateY(-1px);
    box-shadow: 0 3px 14px rgba(255, 255, 255, 0.28);
  }

  .btn-save:active {
    transform: translateY(0);
  }
</style>
