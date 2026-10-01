<script lang="ts">
  import { t, language, setLanguage } from './i18n';
  import { X, Settings2 } from 'lucide-svelte';
  import type { Options } from './types';
  import { defaultOptions, normalizeOptions, SETTINGS_KEY } from './settings';
  import { isTauri } from './tauri-bridge';

  export let options: Options;
  export let onSave: (value: Options) => void;
  export let onClose: () => void;
  let draft = structuredClone(options);
  let error = '';
  let pickingFolder = false;
  $: background = '#' + (draft.jpeg_background ?? [255, 255, 255]).map(n => n.toString(16).padStart(2, '0')).join('');

  function setBackground(event: Event) {
    const hex = (event.target as HTMLInputElement).value;
    draft.jpeg_background = [1, 3, 5].map(offset => parseInt(hex.slice(offset, offset + 2), 16)) as [number, number, number];
  }

  function preset(name: string) {
    const values = name === 'web' ? { quality: 80, max_width: 1920, max_height: 1920, avif_speed: 8 }
      : name === 'quality' ? { quality: 100, max_width: undefined, max_height: undefined, avif_speed: 3 }
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
    } catch (e) {
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
      const elements = Array.from(node.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled)'));
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

<div class="backdrop">
  <section class="panel" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1" use:focusDialog>
    <header>
      <div><span class="eyebrow">VERTEX</span><h2 id="settings-title"><Settings2 size={18} /> {$t("Cài đặt nâng cao")}</h2></div>
      <button type="button" class="icon" on:click={onClose} disabled={pickingFolder} aria-label={$t("Đóng cài đặt")}><X size={18} /></button>
    </header>
    <form on:submit|preventDefault={save}>
      <div class="body">
        <label>{$t("Ngôn ngữ")}<select value={$language} on:change={async (event) => { try { await setLanguage(event.currentTarget.value as 'vi' | 'en'); error = ''; } catch { error = $t("Không thể đổi ngôn ngữ. Hãy thử lại."); } }}><option value="vi">Tiếng Việt</option><option value="en">English</option></select></label>
        <p class="intro">{$t("Áp dụng cho các lần chuyển đổi tiếp theo. Cài đặt được lưu trên thiết bị.")}</p>
        <div class="presets" aria-label={$t("Cấu hình nhanh")}>
          <button type="button" on:click={() => preset('balanced')}>{$t("Cân bằng")}</button>
          <button type="button" on:click={() => preset('web')}>{$t("Nhẹ cho web")}</button>
          <button type="button" on:click={() => preset('quality')}>{$t("Chất lượng cao")}</button>
        </div>
        <fieldset>
          <legend>{$t("Ảnh & kích thước")}</legend>
          <label>{$t("Chất lượng JPG / AVIF")} <strong>{draft.quality}%</strong><input type="range" min="1" max="100" bind:value={draft.quality} /></label>
          <p class="hint">{$t("WEBP luôn được xuất lossless. PNG và TIFF giữ nguyên chất lượng mã hóa.")}</p>
          <div class="columns">
            <label>{$t("Rộng tối đa (px)")}<input type="number" min="1" max="32768" step="1" placeholder={$t("Giữ nguyên")} bind:value={draft.max_width} /></label>
            <label>{$t("Cao tối đa (px)")}<input type="number" min="1" max="32768" step="1" placeholder={$t("Giữ nguyên")} bind:value={draft.max_height} /></label>
          </div>
          <p class="hint">{$t("Ảnh giữ tỷ lệ, chỉ thu nhỏ. Để trống để giữ kích thước gốc. Giới hạn cũng được gửi tới bộ chuyển đổi video.")}</p>
        </fieldset>
        <fieldset>
          <legend>{$t("Độ trong suốt & mã hóa")}</legend>
          <label class="color-row">{$t("Màu nền JPG")} <input type="color" value={background} on:input={setBackground} /></label>
          <p class="hint">{$t("JPG không có nền trong suốt. Các định dạng hỗ trợ alpha vẫn giữ nền trong suốt.")}</p>
          <label>{$t("Ngưỡng alpha GIF")} <strong>{draft.gif_alpha_threshold} / 255</strong><input type="range" min="1" max="255" bind:value={draft.gif_alpha_threshold} /></label>
          <p class="hint">{$t("Pixel dưới ngưỡng sẽ trong suốt. GIF chỉ hỗ trợ trong suốt hoàn toàn hoặc đục hoàn toàn.")}</p>
          <label>{$t("Tốc độ mã hóa AVIF")} <strong>{draft.avif_speed} / 10</strong><input type="range" min="1" max="10" bind:value={draft.avif_speed} /></label>
          <p class="hint">{$t("1: chậm, nén hiệu quả hơn · 10: nhanh hơn.")}</p>
        </fieldset>
        <fieldset>
          <legend>{$t("Lưu file")}</legend>
          <label>{$t("Thư mục đầu ra")}<input readonly value={draft.output_dir || $t("Cùng thư mục với file gốc")} /></label>
          <div class="presets">
            <button type="button" on:click={chooseFolder} disabled={!isTauri || pickingFolder}>{pickingFolder ? $t("Đang chọn…") : $t("Chọn thư mục")}</button>
            <button type="button" on:click={() => draft.output_dir = undefined}>{$t("Dùng thư mục gốc")}</button>
          </div>
          <label>{$t("Khi trùng tên file")}<select bind:value={draft.collision_policy}>
            <option value="rename_with_suffix">{$t("Thêm hậu tố (1), (2)…")}</option>
            <option value="fail_if_exists">{$t("Báo lỗi, giữ file cũ")}</option>
            <option value="overwrite">{$t("Ghi đè file cũ")}</option>
          </select></label>
          {#if draft.collision_policy === 'overwrite'}<p class="warning">{$t("File đầu ra đã tồn tại sẽ bị thay thế.")}</p>{/if}
        </fieldset>
        {#if error}<p role="alert" class="warning">{error}</p>{/if}
      </div>
      <footer>
        <button type="button" on:click={() => draft = defaultOptions()}>{$t("Mặc định")}</button>
        <div><button type="button" on:click={onClose} disabled={pickingFolder}>{$t("Hủy")}</button><button class="primary" type="submit" disabled={pickingFolder}>{$t("Lưu cài đặt")}</button></div>
      </footer>
    </form>
  </section>
</div>

<style>
  .backdrop { position: fixed; inset: 0; display: grid; place-items: center; z-index: 200; background: #08090bd9; pointer-events: auto; padding: 12px; }
  .panel { width: min(420px, 100%); max-height: calc(100vh - 24px); display: flex; flex-direction: column; background: #111216; color: #f4f4f5; border: 1px solid #ffffff20; border-radius: 18px; box-shadow: 0 20px 60px #0009; font-family: var(--font-family); overflow: hidden; }
  header, footer { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 14px 18px; flex-shrink: 0; }
  header { border-bottom: 1px solid #ffffff12; }
  .eyebrow { font-size: 9px; letter-spacing: 1.8px; color: #aaaebc; }
  h2 { display: flex; align-items: center; gap: 8px; font-size: 16px; margin: 5px 0 0; }
  form { min-height: 0; display: flex; flex-direction: column; }
  .body { overflow-y: auto; min-height: 0; padding: 14px 18px; }
  .intro, .hint { font-size: 11px; color: #a7a9b5; line-height: 1.5; margin: 0 0 10px; }
  fieldset { border: 0; border-top: 1px solid #ffffff15; padding: 14px 0 4px; margin: 18px 0 0; min-width: 0; }
  legend { font-size: 12px; font-weight: 700; padding-right: 10px; }
  label { display: block; font-size: 12px; margin-bottom: 10px; }
  strong { float: right; font-size: 11px; color: #c4c8ff; }
  input, select { display: block; width: 100%; box-sizing: border-box; margin-top: 6px; background: #1b1d24; border: 1px solid #ffffff20; border-radius: 7px; color: #f4f4f5; padding: 8px; font: inherit; }
  input[type=range] { accent-color: #c4c8ff; padding: 0; }
  .columns { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .color-row { display: flex; align-items: center; justify-content: space-between; }
  input[type=color] { width: 54px; height: 32px; padding: 3px; margin: 0; }
  button { background: #24262f; color: #eee; border: 1px solid #ffffff15; padding: 7px 10px; border-radius: 8px; font: inherit; font-size: 11px; cursor: pointer; }
  button:hover { background: #343744; }
  button:disabled { opacity: .45; cursor: default; }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid #c4c8ff; outline-offset: 2px; }
  .icon { display: grid; place-items: center; padding: 6px; }
  .presets, footer > div { display: flex; gap: 6px; flex-wrap: wrap; }
  footer { border-top: 1px solid #ffffff12; }
  .primary { background: #d9dcff; color: #151623; font-weight: 700; }
  .primary:hover { background: #f0f1ff; }
  .warning { font-size: 11px; color: #f5ba8b; line-height: 1.5; }
</style>
