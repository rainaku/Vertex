<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { t, language, setLanguage } from './i18n';
  import type { Options } from './types';
  import { defaultOptions, normalizeOptions, SETTINGS_KEY } from './settings';
  import { isTauri } from './tauri-bridge';
  import { updateStore, checkForUpdates, installUpdate, openReleasePage } from './updater';
  import ElasticSlider from './ElasticSlider.svelte';
  import { gsap } from 'gsap';
  import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
  import SlidersHorizontalIcon from 'phosphor-svelte/lib/SlidersHorizontalIcon';
  import ImageIcon from 'phosphor-svelte/lib/ImageIcon';
  import VideoCameraIcon from 'phosphor-svelte/lib/VideoCameraIcon';
  import GearSixIcon from 'phosphor-svelte/lib/GearSixIcon';
  import KeyboardIcon from 'phosphor-svelte/lib/KeyboardIcon';
  import InfoIcon from 'phosphor-svelte/lib/InfoIcon';
  import GlobeHemisphereWestIcon from 'phosphor-svelte/lib/GlobeHemisphereWestIcon';
  import GithubLogoIcon from 'phosphor-svelte/lib/GithubLogoIcon';
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import ArrowSquareOutIcon from 'phosphor-svelte/lib/ArrowSquareOutIcon';
  import ArrowsClockwiseIcon from 'phosphor-svelte/lib/ArrowsClockwiseIcon';
  import ArrowCounterClockwiseIcon from 'phosphor-svelte/lib/ArrowCounterClockwiseIcon';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';

  export let options: Options;
  export let onSave: (value: Options) => void;
  export let onClose: () => void;

  let draft = normalizeOptions(structuredClone(options));
  let error = '';
  let autostartEnabled = false;
  let autostartBusy = true;
  let autostartLoaded = false;
  let autostartError = false;

  async function refreshAutostart() {
    if (!isTauri) {
      autostartBusy = false;
      return;
    }
    autostartBusy = true;
    autostartError = false;
    try {
      autostartEnabled = await isEnabled();
      autostartLoaded = true;
    } catch {
      autostartLoaded = false;
      autostartError = true;
    } finally {
      autostartBusy = false;
    }
  }

  async function toggleAutostart(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const requested = input.checked;
    input.checked = autostartEnabled;
    if (!isTauri || autostartBusy || !autostartLoaded) return;
    autostartBusy = true;
    autostartError = false;
    try {
      if (requested) await enable();
      else await disable();
      autostartEnabled = await isEnabled();
      autostartError = autostartEnabled !== requested;
    } catch {
      autostartError = true;
      // A write may have succeeded even if the verification failed.
      autostartLoaded = false;
    } finally {
      autostartBusy = false;
    }
  }

  function handleWindowFocus() {
    if (!autostartBusy) void refreshAutostart();
  }

  let pickingFolder = false;
  let isClosing = false;
  let appliedSuccess = false;
  let appliedTimer: ReturnType<typeof setTimeout> | undefined;
  let panelEl: HTMLElement | null = null;
  let backdropEl: HTMLElement | null = null;

  // ── Navigation tabs (V-Notch Style) ──────────────────────────────────────────
  type TabKey = 'general' | 'image' | 'video' | 'advanced' | 'shortcuts' | 'about';
  let activeTab: TabKey = 'general';
  let searchQuery = '';

  const TABS: { id: TabKey; labelVi: string; labelEn: string; icon: string }[] = [
    { id: 'general', labelVi: 'Chung', labelEn: 'General', icon: 'general' },
    { id: 'image', labelVi: 'Hình ảnh', labelEn: 'Image Quality', icon: 'image' },
    { id: 'video', labelVi: 'Video & Audio', labelEn: 'Video & Audio', icon: 'video' },
    { id: 'advanced', labelVi: 'Nâng cao', labelEn: 'Advanced', icon: 'advanced' },
    { id: 'shortcuts', labelVi: 'Phím tắt', labelEn: 'Shortcuts', icon: 'shortcuts' },
    { id: 'about', labelVi: 'Thông tin', labelEn: 'About', icon: 'about' },
  ];

  let paneEl: HTMLElement | null = null;
  function selectTab(id: TabKey) {
    if (isClosing) return;
    activeTab = id;
    searchQuery = '';
  }

  // ── GSAP Animations ─────────────────────────────────────────────────────────

  function gsapBackdrop(node: HTMLElement) {
    backdropEl = node;
    gsap.fromTo(node, { opacity: 0 }, { opacity: 1, duration: 0.18, ease: 'power2.out' });
  }

  function gsapPanel(node: HTMLElement) {
    panelEl = node;
    gsap.killTweensOf(node);

    const tl = gsap.timeline();
    tl.fromTo(
      node,
      { opacity: 0, scale: 0.96, y: 8 },
      { opacity: 1, scale: 1, y: 0, duration: 0.28, ease: 'back.out(1.1)' }
    );
  }

  function gsapPane(node: HTMLElement) {
    paneEl = node;
    // Reset then animate in
    gsap.set(node, { opacity: 0, x: 12 });
    gsap.to(node, { opacity: 1, x: 0, duration: 0.2, ease: 'power2.out' });
    return {
      destroy() {
        gsap.killTweensOf(node);
        if (paneEl === node) paneEl = null;
      }
    };
  }

  export function requestClose() {
    if (isClosing || pickingFolder) return;
    isClosing = true;

    const tl = gsap.timeline({
      onComplete: onClose
    });

    if (panelEl) {
      gsap.killTweensOf(panelEl);
      tl.to(panelEl, { opacity: 0, scale: 0.95, y: 6, duration: 0.18, ease: 'power2.in' }, 0);
    }

    if (backdropEl) {
      gsap.killTweensOf(backdropEl);
      tl.to(backdropEl, { opacity: 0, duration: 0.16, ease: 'power2.in' }, 0);
    }
  }

  function handleClose() {
    if (pickingFolder || isClosing) return;
    requestClose();
  }

  function persistDraft(): boolean {
    const value = normalizeOptions(draft);
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(value));
      onSave(value);
      draft = value;
      error = '';
      return true;
    } catch {
      error = $t('Không thể lưu cài đặt trên thiết bị này.');
      return false;
    }
  }

  function handleApply() {
    if (isClosing || pickingFolder) return;
    if (persistDraft()) {
      appliedSuccess = true;
      if (appliedTimer) clearTimeout(appliedTimer);
      appliedTimer = setTimeout(() => {
        appliedSuccess = false;
      }, 1600);
    }
  }

  function handleSave() {
    if (isClosing || pickingFolder) return;
    if (persistDraft()) requestClose();
  }

  function resetToDefault() {
    if (isClosing || pickingFolder) return;
    draft = defaultOptions();
    appliedSuccess = false;
    clearTimeout(appliedTimer);
    error = '';
  }

  async function openLink(url: string) {
    error = '';
    try {
      await openReleasePage(url);
    } catch {
      error = $language === 'vi'
        ? 'Không thể mở liên kết. Hãy thử lại.'
        : 'Could not open the link. Try again.';
    }
  }

  // ── Clean Window Dragging ───────────────────────────────────────────────────

  async function handleHeaderPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    if (target.closest('button, input, select, textarea, a')) return;

    if (isTauri) {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await getCurrentWindow().startDragging();
      } catch (err) {
        console.warn('Window drag failed:', err);
      }
    }
  }

  // ── Global Keyboard Handling ────────────────────────────────────────────────

  function handleGlobalKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      if (searchQuery) {
        searchQuery = '';
      } else {
        handleClose();
      }
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleGlobalKeyDown, true);
    window.addEventListener('focus', handleWindowFocus);
    void refreshAutostart();
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleGlobalKeyDown, true);
    window.removeEventListener('focus', handleWindowFocus);
    if (appliedTimer) clearTimeout(appliedTimer);
    [panelEl, backdropEl, paneEl].forEach(node => { if (node) gsap.killTweensOf(node); });
  });

  // ── Settings Computations ───────────────────────────────────────────────────

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
    if (!isTauri || pickingFolder || isClosing) return;
    pickingFolder = true;
    error = '';
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false, title: $t('Thư mục đầu ra') });
      if (typeof selected === 'string') draft.output_dir = selected;
    } catch {
      error = $t('Không thể chọn thư mục. Hãy thử lại.');
    } finally {
      pickingFolder = false;
    }
  }

  // ── Search matches ──────────────────────────────────────────────────────────

  $: isSearching = searchQuery.trim().length > 0;
  $: lowerSearch = searchQuery.toLowerCase().trim();

  function matchText(text: string, query: string): boolean {
    return text.toLowerCase().includes(query);
  }

  $: hasMatchAutostart = matchText('khởi động cùng windows', lowerSearch) || matchText('startup', lowerSearch) || matchText('autostart', lowerSearch) || matchText('boot', lowerSearch) || matchText('start with windows', lowerSearch);
  $: hasMatchLanguage = matchText('ngôn ngữ', lowerSearch) || matchText('language', lowerSearch) || matchText('tiếng việt', lowerSearch) || matchText('english', lowerSearch);
  $: hasMatchQuality = matchText('chất lượng', lowerSearch) || matchText('quality', lowerSearch) || matchText('jpg', lowerSearch) || matchText('avif', lowerSearch) || matchText('nén', lowerSearch);
  $: hasMatchFolder = matchText('thư mục', lowerSearch) || matchText('folder', lowerSearch) || matchText('output', lowerSearch) || matchText('đầu ra', lowerSearch) || matchText('đích', lowerSearch);
  $: hasMatchCollision = matchText('trùng', lowerSearch) || matchText('tên tệp', lowerSearch) || matchText('collision', lowerSearch) || matchText('ghi đè', lowerSearch) || matchText('overwrite', lowerSearch);
  $: hasMatchMetadata = matchText('exif', lowerSearch) || matchText('siêu dữ liệu', lowerSearch) || matchText('metadata', lowerSearch) || matchText('gps', lowerSearch) || matchText('riêng tư', lowerSearch);
  $: hasMatchBgColor = matchText('màu nền', lowerSearch) || matchText('nền', lowerSearch) || matchText('background', lowerSearch) || matchText('color', lowerSearch);
  $: hasMatchDimensions = matchText('kích thước', lowerSearch) || matchText('rộng', lowerSearch) || matchText('cao', lowerSearch) || matchText('width', lowerSearch) || matchText('height', lowerSearch) || matchText('dimension', lowerSearch);
  $: hasMatchVideoCodec = matchText('video', lowerSearch) || matchText('codec', lowerSearch) || matchText('h264', lowerSearch) || matchText('h265', lowerSearch) || matchText('mã hóa', lowerSearch);
  $: hasMatchVideoCrf = matchText('crf', lowerSearch) || matchText('nét', lowerSearch) || matchText('dung lượng video', lowerSearch);
  $: hasMatchAvifSpeed = matchText('avif', lowerSearch) || matchText('tốc độ', lowerSearch) || matchText('speed', lowerSearch);
  $: hasMatchGifAlpha = matchText('gif', lowerSearch) || matchText('alpha', lowerSearch) || matchText('trong suốt', lowerSearch) || matchText('threshold', lowerSearch);
  $: hasMatchDpi = matchText('dpi', lowerSearch) || matchText('pdf', lowerSearch) || matchText('tài liệu', lowerSearch) || matchText('phân giải', lowerSearch);
  $: hasMatchVideoPreset = matchText('preset', lowerSearch) || matchText('cpu', lowerSearch) || matchText('fast', lowerSearch) || matchText('slow', lowerSearch);
  $: hasMatchShortcuts = matchText('phím tắt', lowerSearch) || matchText('shortcut', lowerSearch) || matchText('shift', lowerSearch) || matchText('drag', lowerSearch) || matchText('esc', lowerSearch);

  $: hasAnySearchResult =
    hasMatchAutostart ||
    hasMatchLanguage ||
    hasMatchQuality ||
    hasMatchFolder ||
    hasMatchCollision ||
    hasMatchMetadata ||
    hasMatchBgColor ||
    hasMatchDimensions ||
    hasMatchVideoCodec ||
    hasMatchVideoCrf ||
    hasMatchAvifSpeed ||
    hasMatchGifAlpha ||
    hasMatchDpi ||
    hasMatchVideoPreset ||
    hasMatchShortcuts;
</script>
{#snippet videoPresetControl()}
              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Tốc độ CPU encode (Preset)' : 'Encoding speed preset'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Tỷ lệ đánh đổi giữa tốc độ xử lý và kích thước nén.' : 'Tradeoff between processing speed and file size.'}</span>
                </div>
                <div class="seg">
                  <button type="button" class="seg-btn" class:seg-on={draft.video_preset === 'fast'} on:click={() => draft.video_preset = 'fast'}>Fast</button>
                  <button type="button" class="seg-btn" class:seg-on={draft.video_preset === 'medium'} on:click={() => draft.video_preset = 'medium'}>Medium</button>
                  <button type="button" class="seg-btn" class:seg-on={draft.video_preset === 'slow'} on:click={() => draft.video_preset = 'slow'}>Slow</button>
                </div>
              </div>
{/snippet}
{#snippet shortcutControls()}

              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Shift + Kéo tệp' : 'Shift + Drag & Drop'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Giữ phím Shift trong khi kéo tệp bất kỳ từ Explorer để triệu hồi vòng quay chuyển đổi tức thì.' : 'Hold Shift while dragging files from Explorer to instantly summon the radial wheel.'}</span>
                </div>
                <span class="badge-key">Shift + Drag</span>
              </div>

              <div class="divider"></div>

              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Hủy nhanh (Escape)' : 'Quick dismiss (Escape)'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Nhấn Esc hoặc click ra ngoài để đóng ngay cửa sổ.' : 'Press Esc or click outside to dismiss.'}</span>
                </div>
                <span class="badge-key">Esc</span>
              </div>
{/snippet}


<div
  class="backdrop"
  role="presentation"
  on:click|self={handleClose}
  use:gsapBackdrop
>
  <div class="panel-shadow" use:gsapPanel>
  <div
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-labelledby="settings-title"
    tabindex="-1"
  >
    <!-- ── Shell Layout (Sidebar + Main Pane) ────────────────────────────────── -->
    <div class="shell">
      <!-- ── Left Sidebar (V-Notch Style) ───────────────────────────────────── -->
      <aside class="sidebar">
        <!-- Search Box -->
        <div class="search-box">
          <MagnifyingGlassIcon class="search-icon" size={16} weight="bold" aria-hidden="true" />
          <input
            type="text"
            class="search-input"
            placeholder={$language === 'vi' ? 'Tìm kiếm cài đặt…' : 'Search settings…'}
            aria-label={$language === 'vi' ? 'Tìm kiếm cài đặt' : 'Search settings'}
            bind:value={searchQuery}
          />
          {#if searchQuery}
            <button type="button" class="btn-clear-search" on:click={() => searchQuery = ''} aria-label={$language === 'vi' ? 'Xóa tìm kiếm' : 'Clear search'}>
              <XIcon size={14} weight="bold" aria-hidden="true" />
            </button>
          {/if}
        </div>

        <!-- Navigation items list -->
        <nav class="nav-list" aria-label="Settings Categories">
          {#if isSearching}
            <div class="nav-item nav-active">
              <span class="nav-icon">
                <MagnifyingGlassIcon size={18} weight="bold" aria-hidden="true" />
              </span>
              <span class="nav-label">{$language === 'vi' ? 'Kết quả tìm kiếm' : 'Search results'}</span>
            </div>
          {:else}
            {#each TABS as tab}
              <button
                type="button"
                class="nav-item"
                class:nav-active={activeTab === tab.id}
                aria-current={activeTab === tab.id ? 'page' : undefined}
                on:click={() => selectTab(tab.id)}
              >
                <span class="nav-icon">
                  {#if tab.icon === 'general'}
                    <SlidersHorizontalIcon size={18} weight="fill" aria-hidden="true" />
                  {:else if tab.icon === 'image'}
                    <ImageIcon size={18} weight="fill" aria-hidden="true" />
                  {:else if tab.icon === 'video'}
                    <VideoCameraIcon size={18} weight="fill" aria-hidden="true" />
                  {:else if tab.icon === 'advanced'}
                    <GearSixIcon size={18} weight="fill" aria-hidden="true" />
                  {:else if tab.icon === 'shortcuts'}
                    <KeyboardIcon size={18} weight="fill" aria-hidden="true" />
                  {:else}
                    <InfoIcon size={18} weight="fill" aria-hidden="true" />
                  {/if}
                </span>
                <span class="nav-label">{$language === 'vi' ? tab.labelVi : tab.labelEn}</span>
                {#if tab.id === 'about' && $updateStore.status === 'available'}
                  <span class="nav-update-badge">NEW</span>
                {/if}
              </button>
            {/each}
          {/if}
        </nav>

        <!-- Sidebar Footer (V-Notch Style) -->
        <div class="sidebar-footer">
          <div class="sidebar-socials">
            <button type="button" class="btn-social" title="Vertex Website" aria-label="Vertex Website" on:click={() => openLink('https://rainaku.id.vn')}>
              <GlobeHemisphereWestIcon size={18} weight="fill" aria-hidden="true" />
            </button>
            <button type="button" class="btn-social" title="GitHub" aria-label="GitHub" on:click={() => openLink('https://github.com/rainaku/Vertex')}>
              <GithubLogoIcon size={18} weight="fill" aria-hidden="true" />
            </button>
          </div>
          <span class="build-tag">Build {$updateStore.currentVersion}</span>
        </div>
      </aside>

      <!-- ── Right Main Content Area ────────────────────────────────────────── -->
      <div class="content-area">
        <!-- Top Title Bar / Draggable Header -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <header
          class="content-header"
          on:pointerdown={handleHeaderPointerDown}
        >
          <div class="header-left">
            <h2 id="settings-title" class="hero-title">
              {#if isSearching}
                {$language === 'vi' ? 'Tìm kiếm' : 'Search'}
              {:else}
                {$language === 'vi' ? TABS.find(t => t.id === activeTab)?.labelVi : TABS.find(t => t.id === activeTab)?.labelEn}
              {/if}
            </h2>
          </div>

          <button
            type="button"
            class="btn-close"
            on:pointerdown|stopPropagation
            on:click={handleClose}
            disabled={pickingFolder || isClosing}
            aria-label={$t('Đóng cài đặt')}
          >
            <XIcon size={16} weight="bold" aria-hidden="true" />
          </button>
        </header>

        <!-- Scrollable settings list -->
        {#key `${activeTab}::${isSearching}`}
        <div class="content-scroll" use:gsapPane>
          {#if isSearching}
            <div class="section-title">{$language === 'vi' ? 'KẾT QUẢ TÌM KIẾM' : 'SEARCH RESULTS'}</div>
          {/if}
          {#if (!isSearching && activeTab === 'general') || (isSearching && hasMatchAutostart)}
            <div class="card">
              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Khởi động cùng Windows' : 'Start with Windows'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Tự chạy Vertex ở khay hệ thống khi đăng nhập. Thay đổi có hiệu lực ngay.' : 'Launch Vertex in the system tray when you sign in. Changes apply immediately.'}</span>
                  {#if !isTauri}
                    <span class="row-desc">{$language === 'vi' ? 'Chỉ khả dụng trong ứng dụng desktop.' : 'Available in the desktop app only.'}</span>
                  {:else if autostartBusy}
                    <span class="row-desc" role="status">{$language === 'vi' ? 'Đang cập nhật trạng thái…' : 'Updating status…'}</span>
                  {/if}
                </div>
                <label class="switch">
                  <input type="checkbox" checked={autostartEnabled} on:change={toggleAutostart} disabled={!isTauri || autostartBusy || !autostartLoaded} aria-label={$language === 'vi' ? 'Khởi động cùng Windows' : 'Start with Windows'}/>
                  <span class="slider"></span>
                </label>
              </div>
              {#if autostartError}
                <div class="setting-row">
                  <span class="row-desc" role="alert">{$language === 'vi' ? 'Không thể đọc hoặc thay đổi cài đặt khởi động Windows.' : 'Could not read or change the Windows startup setting.'}</span>
                  <button type="button" class="btn-subtle" on:click={refreshAutostart} disabled={autostartBusy}>{$language === 'vi' ? 'Thử lại' : 'Retry'}</button>
                </div>
              {/if}
            </div>
          {/if}
          {#if isSearching}
            <!-- Search results -->
            {#if hasAnySearchResult}
              <div class="card">
                {#if hasMatchLanguage}
                  <div class="setting-row">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Ngôn ngữ giao diện' : 'Display language'}</span>
                      <span class="row-desc">{$language === 'vi' ? 'Tiếng Việt hoặc English.' : 'Switch app language.'}</span>
                    </div>
                    <div class="seg">
                      <button type="button" class="seg-btn" class:seg-on={$language === 'vi'} on:click={() => setLanguage('vi')}>Tiếng Việt</button>
                      <button type="button" class="seg-btn" class:seg-on={$language === 'en'} on:click={() => setLanguage('en')}>English</button>
                    </div>
                  </div>
                {/if}

                {#if hasMatchQuality}
                  {#if hasMatchLanguage}<div class="divider"></div>{/if}
                  <div class="setting-row vertical">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Chất lượng ảnh JPG / AVIF' : 'JPG / AVIF quality'}</span>
                      <span class="row-desc">{$language === 'vi' ? 'Tối ưu độ nén và kích thước tệp.' : 'Image compression ratio.'}</span>
                    </div>
                    <ElasticSlider bind:value={draft.quality} min={1} max={100} step={1} unit="%"/>
                  </div>
                {/if}

                {#if hasMatchFolder}
                  {#if hasMatchLanguage || hasMatchQuality}<div class="divider"></div>{/if}
                  <div class="setting-row vertical">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Thư mục xuất file' : 'Output folder'}</span>
                      <span class="row-desc">{draft.output_dir || ($language === 'vi' ? 'Cùng thư mục file gốc' : 'Same as source')}</span>
                    </div>
                    <div class="folder-row">
                      <div class="folder-path">{draft.output_dir || ($language === 'vi' ? 'Cùng thư mục file gốc' : 'Same folder as source')}</div>
                      <button type="button" class="btn-subtle" on:click={chooseFolder} disabled={!isTauri || pickingFolder}>
                        {pickingFolder ? $t('Đang chọn…') : $t('Duyệt…')}
                      </button>
                      {#if draft.output_dir}
                        <button type="button" class="btn-subtle" on:click={() => draft.output_dir = undefined} disabled={pickingFolder} title={$language === 'vi' ? 'Đặt lại mặc định' : 'Reset to default'} aria-label={$t('Dùng thư mục gốc')}>
                          <ArrowCounterClockwiseIcon size={16} weight="bold" aria-hidden="true" />
                        </button>
                      {/if}
                    </div>
                  </div>
                {/if}

                {#if hasMatchCollision}
                  {#if hasMatchLanguage || hasMatchQuality || hasMatchFolder}<div class="divider"></div>{/if}
                  <div class="setting-row vertical">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Quy tắc trùng tên tệp' : 'Collision policy'}</span>
                      <span class="row-desc">{$language === 'vi' ? 'Hành động khi tệp đầu ra đã tồn tại.' : 'Action when target file exists.'}</span>
                    </div>
                    <div class="seg seg-full">
                      <button type="button" class="seg-btn" class:seg-on={draft.collision_policy === 'rename_with_suffix'} on:click={() => draft.collision_policy = 'rename_with_suffix'}>{$language === 'vi' ? 'Đổi tên (+1)' : 'Rename'}</button>
                      <button type="button" class="seg-btn" class:seg-on={draft.collision_policy === 'fail_if_exists'} on:click={() => draft.collision_policy = 'fail_if_exists'}>{$language === 'vi' ? 'Dừng lại' : 'Stop'}</button>
                      <button type="button" class="seg-btn seg-danger" class:seg-on={draft.collision_policy === 'overwrite'} on:click={() => draft.collision_policy = 'overwrite'}>{$language === 'vi' ? 'Ghi đè' : 'Overwrite'}</button>
                    </div>
                  </div>
                {/if}

                {#if hasMatchMetadata}
                  {#if hasMatchLanguage || hasMatchQuality || hasMatchFolder || hasMatchCollision}<div class="divider"></div>{/if}
                  <div class="setting-row">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Xóa siêu dữ liệu (EXIF / GPS)' : 'Strip metadata (EXIF / GPS)'}</span>
                      <span class="row-desc">{$language === 'vi' ? 'Bảo vệ quyền riêng tư cá nhân.' : 'Privacy protection.'}</span>
                    </div>
                    <label class="switch">
                      <input type="checkbox" bind:checked={draft.strip_metadata}/>
                      <span class="slider"></span>
                    </label>
                  </div>
                {/if}

                {#if hasMatchBgColor}
                  {#if hasMatchLanguage || hasMatchQuality || hasMatchFolder || hasMatchCollision || hasMatchMetadata}<div class="divider"></div>{/if}
                  <div class="setting-row">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Màu nền mặc định cho JPG' : 'JPG background color'}</span>
                      <span class="row-desc">{$language === 'vi' ? 'Thay thế cho nền trong suốt.' : 'Fills transparent backgrounds.'}</span>
                    </div>
                    <label class="swatch-pill">
                      <span class="swatch-dot" style="background:{backgroundHex};"></span>
                      <span class="swatch-hex">{backgroundHex.toUpperCase()}</span>
                      <input type="color" value={backgroundHex} on:input={setBackground} class="color-hidden"/>
                    </label>
                  </div>
                {/if}

                {#if hasMatchDimensions}
                  {#if hasMatchLanguage || hasMatchQuality || hasMatchFolder || hasMatchCollision || hasMatchMetadata || hasMatchBgColor}<div class="divider"></div>{/if}
                  <div class="setting-row vertical">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Giới hạn kích thước ảnh (W/H)' : 'Max dimensions (W/H)'}</span>
                      <span class="row-desc">{$language === 'vi' ? 'Thu nhỏ ảnh nếu vượt quá giới hạn.' : 'Scale down only if larger.'}</span>
                    </div>
                    <div class="twin-inputs">
                      <div class="input-cell"><span class="input-tag">W</span><input type="number" min="1" max="32768" step="1" placeholder="Auto" bind:value={draft.max_width} class="num-input"/></div>
                      <div class="input-cell"><span class="input-tag">H</span><input type="number" min="1" max="32768" step="1" placeholder="Auto" bind:value={draft.max_height} class="num-input"/></div>
                    </div>
                  </div>
                {/if}

                {#if hasMatchVideoCodec}
                  <div class="divider"></div>
                  <div class="setting-row">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Bộ mã hóa video' : 'Video codec'}</span>
                      <span class="row-desc">{draft.video_codec === 'h265' ? 'H.265 (HEVC)' : 'H.264 (AVC)'}</span>
                    </div>
                    <div class="seg">
                      <button type="button" class="seg-btn" class:seg-on={draft.video_codec === 'h264'} on:click={() => draft.video_codec = 'h264'}>H.264</button>
                      <button type="button" class="seg-btn" class:seg-on={draft.video_codec === 'h265'} on:click={() => draft.video_codec = 'h265'}>H.265</button>
                    </div>
                  </div>
                {/if}

                {#if hasMatchVideoCrf}
                  <div class="divider"></div>
                  <div class="setting-row vertical">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Chất lượng video (CRF)' : 'Constant Rate Factor (CRF)'}</span>
                      <span class="row-desc">{$language === 'vi' ? 'CRF thấp hơn = chi tiết nét hơn.' : 'Lower CRF = sharper details.'}</span>
                    </div>
                    <ElasticSlider bind:value={draft.video_crf} min={18} max={35} step={1} unit="CRF"/>
                  </div>
                {/if}

                {#if hasMatchAvifSpeed}
                  <div class="divider"></div>
                  <div class="setting-row vertical">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Tốc độ mã hóa AVIF' : 'AVIF encoding speed'}</span>
                      <span class="row-desc">{$language === 'vi' ? '1: Tốt nhất (chậm) · 10: Nhanh nhất.' : '1 = best · 10 = fastest.'}</span>
                    </div>
                    <ElasticSlider bind:value={draft.avif_speed} min={1} max={10} step={1} unit="/ 10"/>
                  </div>
                {/if}

                {#if hasMatchGifAlpha}
                  <div class="divider"></div>
                  <div class="setting-row vertical">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Ngưỡng trong suốt alpha GIF' : 'GIF alpha threshold'}</span>
                      <span class="row-desc">{$language === 'vi' ? 'Ngưỡng loại bỏ độ mờ.' : 'Alpha opacity cutoff.'}</span>
                    </div>
                    <ElasticSlider bind:value={draft.gif_alpha_threshold} min={1} max={255} step={1} unit="/ 255"/>
                  </div>
                {/if}

                {#if hasMatchVideoPreset}{@render videoPresetControl()}{/if}
                {#if hasMatchShortcuts}{@render shortcutControls()}{/if}
                {#if hasMatchDpi}
                  <div class="divider"></div>
                  <div class="setting-row vertical">
                    <div class="row-info">
                      <span class="row-title">{$language === 'vi' ? 'Độ phân giải xuất PDF (DPI)' : 'PDF render DPI'}</span>
                      <span class="row-desc">{$language === 'vi' ? 'Mật độ điểm ảnh khi trích xuất trang PDF.' : 'Pixel density for PDF rasterization.'}</span>
                    </div>
                    <ElasticSlider bind:value={draft.dpi} min={72} max={600} step={10} unit="DPI"/>
                  </div>
                {/if}
              </div>
            {:else if !hasMatchAutostart}
              <div class="card empty-search-card">
                <span class="empty-title">{$language === 'vi' ? 'Không tìm thấy kết quả' : 'No settings found'}</span>
                <span class="empty-sub">{$language === 'vi' ? `Không có tùy chọn nào khớp với "${searchQuery}"` : `No options matching "${searchQuery}"`}</span>
              </div>
            {/if}

          {:else if activeTab === 'general'}
            <!-- ── TAB: GENERAL ─────────────────────────────────────────────── -->
            <div class="card">
              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Ngôn ngữ giao diện' : 'Display Language'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Thay đổi ngôn ngữ hiển thị trên toàn bộ giao diện.' : 'Select language for the user interface.'}</span>
                </div>
                <div class="seg">
                  <button type="button" class="seg-btn" class:seg-on={$language === 'vi'} on:click={() => setLanguage('vi')}>Tiếng Việt</button>
                  <button type="button" class="seg-btn" class:seg-on={$language === 'en'} on:click={() => setLanguage('en')}>English</button>
                </div>
              </div>
            </div>

            <div class="section-title">{$language === 'vi' ? 'CẤU HÌNH NHANH' : 'PRESETS'}</div>
            <div class="card">
              <div class="setting-row vertical">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Hồ sơ chuyển đổi' : 'Optimized profiles'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Áp dụng nhanh các thiết lập độ nét và kích thước.' : 'Quickly apply balanced or maximum quality.'}</span>
                </div>
                <div class="chips">
                  <button type="button" class="chip" class:chip-on={activePreset === 'balanced'} on:click={() => preset('balanced')}>
                    {$language === 'vi' ? 'Cân bằng' : 'Balanced'}
                  </button>
                  <button type="button" class="chip" class:chip-on={activePreset === 'web'} on:click={() => preset('web')}>
                    {$language === 'vi' ? 'Nhẹ cho web' : 'Web compact'}
                  </button>
                  <button type="button" class="chip" class:chip-on={activePreset === 'quality'} on:click={() => preset('quality')}>
                    {$language === 'vi' ? 'Chất lượng cao' : 'High quality'}
                  </button>
                </div>
              </div>
            </div>

            <div class="section-title">{$language === 'vi' ? 'THƯ MỤC & ĐÍCH ĐẾN' : 'OUTPUT DESTINATION'}</div>
            <div class="card">
              <div class="setting-row vertical">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Thư mục xuất file' : 'Output folder'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Mặc định lưu cùng thư mục với tệp gốc.' : 'Defaults to the same folder as the original file.'}</span>
                </div>
                <div class="folder-row">
                  <div class="folder-path" title={draft.output_dir || ($language === 'vi' ? 'Cùng thư mục với file gốc' : 'Same folder as source')}>
                    <span class="path-text">{draft.output_dir || ($language === 'vi' ? 'Cùng thư mục với file gốc' : 'Same folder as source')}</span>
                  </div>
                  <button type="button" class="btn-subtle" on:click={chooseFolder} disabled={!isTauri || pickingFolder}>
                    {pickingFolder ? '…' : ($language === 'vi' ? 'Duyệt…' : 'Browse…')}
                  </button>
                  {#if draft.output_dir}
                    <button type="button" class="btn-subtle" on:click={() => draft.output_dir = undefined} disabled={pickingFolder} title={$language === 'vi' ? 'Đặt lại mặc định' : 'Reset to default'} aria-label={$t('Dùng thư mục gốc')}>
                      <ArrowCounterClockwiseIcon size={16} weight="bold" aria-hidden="true" />
                    </button>
                  {/if}
                </div>
              </div>

              <div class="divider"></div>

              <div class="setting-row vertical">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Quy tắc trùng tên tệp' : 'Collision policy'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Hành động khi tệp đầu ra đã tồn tại trên đĩa.' : 'Behavior when output file already exists.'}</span>
                </div>
                <div class="seg seg-full">
                  <button type="button" class="seg-btn" class:seg-on={draft.collision_policy === 'rename_with_suffix'} on:click={() => draft.collision_policy = 'rename_with_suffix'}>
                    {$language === 'vi' ? 'Đổi tên (+1)' : 'Rename'}
                  </button>
                  <button type="button" class="seg-btn" class:seg-on={draft.collision_policy === 'fail_if_exists'} on:click={() => draft.collision_policy = 'fail_if_exists'}>
                    {$language === 'vi' ? 'Dừng lại' : 'Stop'}
                  </button>
                  <button type="button" class="seg-btn seg-danger" class:seg-on={draft.collision_policy === 'overwrite'} on:click={() => draft.collision_policy = 'overwrite'}>
                    {$language === 'vi' ? 'Ghi đè' : 'Overwrite'}
                  </button>
                </div>
              </div>
            </div>

          {:else if activeTab === 'image'}
            <!-- ── TAB: IMAGE QUALITY ───────────────────────────────────────── -->
            <div class="section-title">{$language === 'vi' ? 'CHẤT LƯỢNG & NÉN HÌNH ẢNH' : 'IMAGE QUALITY & COMPRESSION'}</div>
            <div class="card">
              <div class="setting-row vertical">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Chất lượng JPG / AVIF' : 'JPG / AVIF quality'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'WEBP luôn lossless. PNG và TIFF bảo toàn nguyên gốc.' : 'WEBP is lossless. PNG and TIFF remain unaffected.'}</span>
                </div>
                <ElasticSlider bind:value={draft.quality} min={1} max={100} step={1} unit="%"/>
              </div>

              <div class="divider"></div>

              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Xóa siêu dữ liệu (EXIF / GPS)' : 'Strip metadata (EXIF / GPS)'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Bảo vệ vị trí chụp, thông tin thiết bị và quyền riêng tư.' : 'Protect camera info, geolocation and privacy.'}</span>
                </div>
                <label class="switch">
                  <input type="checkbox" bind:checked={draft.strip_metadata}/>
                  <span class="slider"></span>
                </label>
              </div>

              <div class="divider"></div>

              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Màu nền mặc định cho JPG' : 'JPG background fill color'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'JPG không hỗ trợ kênh trong suốt (alpha).' : 'JPG does not support transparency channel.'}</span>
                </div>
                <label class="swatch-pill">
                  <span class="swatch-dot" style="background:{backgroundHex};"></span>
                  <span class="swatch-hex">{backgroundHex.toUpperCase()}</span>
                  <input type="color" value={backgroundHex} on:input={setBackground} class="color-hidden"/>
                </label>
              </div>
            </div>

            <div class="section-title">{$language === 'vi' ? 'GIỚI HẠN KÍCH THƯỚC (PX)' : 'MAX DIMENSIONS (PX)'}</div>
            <div class="card">
              <div class="setting-row vertical">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Chiều rộng / Chiều cao tối đa' : 'Maximum width & height'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Ảnh chỉ thu nhỏ nếu vượt quá, luôn giữ đúng tỷ lệ khung hình.' : 'Scales down only if larger. Aspect-ratio is always preserved.'}</span>
                </div>
                <div class="twin-inputs">
                  <div class="input-cell">
                    <span class="input-tag">W</span>
                    <input type="number" min="1" max="32768" step="1" placeholder={$language === 'vi' ? 'Giữ nguyên' : 'Auto'} bind:value={draft.max_width} class="num-input"/>
                  </div>
                  <div class="input-cell">
                    <span class="input-tag">H</span>
                    <input type="number" min="1" max="32768" step="1" placeholder={$language === 'vi' ? 'Giữ nguyên' : 'Auto'} bind:value={draft.max_height} class="num-input"/>
                  </div>
                </div>
              </div>
            </div>

          {:else if activeTab === 'video'}
            <!-- ── TAB: VIDEO & AUDIO ───────────────────────────────────────── -->
            <div class="section-title">{$language === 'vi' ? 'MÃ HÓA VIDEO & CRF' : 'VIDEO CODEC & ENCODING'}</div>
            <div class="card">
              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Bộ mã hóa video' : 'Video codec'}</span>
                  <span class="row-desc">{draft.video_codec === 'h265'
                    ? ($language === 'vi' ? 'H.265 (HEVC): Tỷ lệ nén siêu việt, dung lượng nhỏ.' : 'H.265 (HEVC): Superior compression, smaller size.')
                    : ($language === 'vi' ? 'H.264 (AVC): Tương thích tuyệt đối trên mọi thiết bị.' : 'H.264 (AVC): Universal playback compatibility.')}</span>
                </div>
                <div class="seg">
                  <button type="button" class="seg-btn" class:seg-on={draft.video_codec === 'h264'} on:click={() => draft.video_codec = 'h264'}>H.264</button>
                  <button type="button" class="seg-btn" class:seg-on={draft.video_codec === 'h265'} on:click={() => draft.video_codec = 'h265'}>H.265</button>
                </div>
              </div>

              <div class="divider"></div>

              <div class="setting-row vertical">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Chất lượng video (CRF)' : 'Constant Rate Factor (CRF)'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'CRF thấp hơn = chi tiết nét hơn; CRF cao hơn = tệp nhẹ hơn.' : 'Lower CRF = sharper details; higher CRF = smaller file.'}</span>
                </div>
                <ElasticSlider bind:value={draft.video_crf} min={18} max={35} step={1} unit="CRF"/>
              </div>

              <div class="divider"></div>

              <div class="setting-row vertical">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Tốc độ mã hóa AVIF' : 'AVIF encoding speed'}</span>
                  <span class="row-desc">{$language === 'vi' ? '1: Nén tốt nhất (chậm) · 10: Nhanh nhất.' : '1 = best compression (slow) · 10 = fastest.'}</span>
                </div>
                <ElasticSlider bind:value={draft.avif_speed} min={1} max={10} step={1} unit="/ 10"/>
              </div>
            </div>

            <div class="section-title">{$language === 'vi' ? 'ẢNH ĐỘNG GIF' : 'ANIMATED GIF'}</div>
            <div class="card">
              <div class="setting-row vertical">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Ngưỡng trong suốt alpha GIF' : 'GIF alpha threshold'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Các pixel có độ mờ dưới ngưỡng sẽ trở thành hoàn toàn trong suốt.' : 'Pixels below this opacity threshold become transparent.'}</span>
                </div>
                <ElasticSlider bind:value={draft.gif_alpha_threshold} min={1} max={255} step={1} unit="/ 255"/>
              </div>
            </div>

          {:else if activeTab === 'advanced'}
            <!-- ── TAB: ADVANCED / PDF ──────────────────────────────────────── -->
            <div class="section-title">{$language === 'vi' ? 'TÀI LIỆU & PDF' : 'PDF & DOCUMENTS'}</div>
            <div class="card">
              <div class="setting-row vertical">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Độ phân giải xuất PDF (DPI)' : 'PDF render DPI'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Mật độ điểm ảnh khi trích xuất trang PDF thành ảnh (JPG, PNG, WebP).' : 'Pixel density when rendering PDF pages to images.'}</span>
                </div>
                <ElasticSlider bind:value={draft.dpi} min={72} max={600} step={10} unit="DPI"/>
              </div>
            </div>

            <div class="section-title">{$language === 'vi' ? 'TÙY CHỌN NÂNG CAO' : 'ADVANCED PRESETS'}</div>
            <div class="card">
{@render videoPresetControl()}
            </div>

          {:else if activeTab === 'shortcuts'}
            <!-- ── TAB: SHORTCUTS & GESTURES ────────────────────────────────── -->
            <div class="section-title">{$language === 'vi' ? 'THAO TÁC CỬ CHỈ' : 'GESTURES & SHORTCUTS'}</div>
            <div class="card">{@render shortcutControls()}            </div>

          {:else if activeTab === 'about'}
            <!-- ── TAB: ABOUT ───────────────────────────────────────────────── -->
            <div class="card">
              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">Vertex Radial Converter</span>
                  <span class="row-desc">{$language === 'vi' ? 'Bộ công cụ chuyển đổi tệp đa phương tiện tức thì.' : 'Instant radial file conversion toolkit.'}</span>
                </div>
                <span class="badge-key">v{$updateStore.currentVersion}</span>
              </div>
              <div class="divider"></div>
              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Kiến trúc lõi' : 'Core Architecture'}</span>
                  <span class="row-desc">Rust Core Engine + Tauri 2 + Svelte 5 + GSAP</span>
                </div>
                <span class="badge-key">Rust + Tauri</span>
              </div>
              <div class="divider"></div>
              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">GitHub Repository</span>
                  <span class="row-desc">rainaku/Vertex</span>
                </div>
                <button type="button" class="btn-subtle" on:click={() => openLink('https://github.com/rainaku/Vertex')}>
                  <ArrowSquareOutIcon size={16} weight="fill" aria-hidden="true" />
                  <span>{$language === 'vi' ? 'Mở GitHub' : 'Open GitHub'}</span>
                </button>
              </div>
              <div class="divider"></div>
              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Pháp lý & Quyền riêng tư' : 'Legal & Privacy'}</span>
                  <span class="row-desc">{$language === 'vi' ? 'Xử lý cục bộ 100%, không thu thập dữ liệu cá nhân hay telemetry.' : '100% local processing, zero telemetry or personal data tracking.'}</span>
                </div>
                <div class="legal-links">
                  <button type="button" class="btn-subtle" on:click={() => openLink('https://github.com/rainaku/Vertex/blob/main/TERMS_OF_SERVICE.md')}>
                    <span>{$language === 'vi' ? 'Điều khoản' : 'Terms'}</span>
                  </button>
                  <button type="button" class="btn-subtle" on:click={() => openLink('https://github.com/rainaku/Vertex/blob/main/PRIVACY_POLICY.md')}>
                    <span>{$language === 'vi' ? 'Bảo mật' : 'Privacy'}</span>
                  </button>
                </div>
              </div>
            </div>

            <!-- ── Auto Update Card ────────────────────────────────────────── -->
            <div class="section-title">{$language === 'vi' ? 'CẬP NHẬT PHIÊN BẢN' : 'SOFTWARE UPDATES'}</div>
            <div class="card update-card" class:update-card-active={$updateStore.status === 'available'}>
              <div class="setting-row">
                <div class="row-info">
                  <div class="update-header-line">
                    <span class="row-title">{$language === 'vi' ? 'Trạng thái phát hành' : 'Release Status'}</span>
                    {#if $updateStore.status === 'available'}
                      <span class="badge-update-pulse">v{$updateStore.latestVersion} AVAILABLE</span>
                    {/if}
                  </div>
                  <span class="row-desc">
                    {#if $updateStore.status === 'checking'}
                      {$language === 'vi' ? 'Đang kiểm tra phiên bản mới từ GitHub...' : 'Checking for releases on GitHub...'}
                    {:else if $updateStore.status === 'up-to-date'}
                      {$language === 'vi' ? `Bạn đang sử dụng phiên bản mới nhất (v${$updateStore.currentVersion}).` : `You are running the latest version (v${$updateStore.currentVersion}).`}
                    {:else if $updateStore.status === 'available'}
                      {$language === 'vi' ? `Đã có bản cập nhật mới v${$updateStore.latestVersion}! Bạn có thể tải và cài đặt ngay.` : `A new release v${$updateStore.latestVersion} is available!`}
                    {:else if $updateStore.status === 'downloading'}
                      {$language === 'vi' ? `Đang tải bản cập nhật: ${$updateStore.downloadProgress}%` : `Downloading update: ${$updateStore.downloadProgress}%`}
                    {:else if $updateStore.status === 'downloaded'}
                      {$language === 'vi' ? 'Đã tải xong bản cập nhật! Khởi động lại ứng dụng để áp dụng.' : 'Update downloaded! Restart to apply.'}
                    {:else if $updateStore.status === 'error'}
                      <span class="update-error-text">{$updateStore.error || ($language === 'vi' ? 'Lỗi kiểm tra cập nhật.' : 'Update check failed.')}</span>
                    {:else}
                      {$language === 'vi' ? 'Kiểm tra bản cập nhật mới nhất từ kho lưu trữ GitHub.' : 'Check for the latest releases from GitHub.'}
                    {/if}
                  </span>
                </div>

                <div class="update-actions">
                  {#if $updateStore.status === 'available' || $updateStore.status === 'downloading' || $updateStore.status === 'downloaded'}
                    <button
                      type="button"
                      class="btn-update-primary"
                      on:click={installUpdate}
                      disabled={$updateStore.status === 'downloading'}
                    >
                      {#if $updateStore.status === 'downloading'}
                        <span class="spinner-inline"></span>
                        <span>{$updateStore.downloadProgress}%</span>
                      {:else if $updateStore.status === 'downloaded'}
                        {$language === 'vi' ? 'Khởi động lại' : 'Restart Now'}
                      {:else}
                        {$language === 'vi' ? 'Cập nhật ngay' : 'Update Now'}
                      {/if}
                    </button>
                  {/if}

                  <button
                    type="button"
                    class="btn-subtle"
                    on:click={() => checkForUpdates(true)}
                    disabled={$updateStore.status === 'checking' || $updateStore.status === 'downloading'}
                  >
                    {#if $updateStore.status === 'checking'}
                      <span class="spinner-inline"></span>
                      <span>{$language === 'vi' ? 'Đang kiểm tra…' : 'Checking…'}</span>
                    {:else}
                      <ArrowsClockwiseIcon size={16} weight="bold" aria-hidden="true" />
                      <span>{$language === 'vi' ? 'Kiểm tra cập nhật' : 'Check for Updates'}</span>
                    {/if}
                  </button>
                </div>
              </div>

              <!-- Download progress bar -->
              {#if $updateStore.status === 'downloading'}
                <div class="update-progress-container">
                  <div class="update-progress-bar" style="width: {$updateStore.downloadProgress}%;"></div>
                </div>
              {/if}

              <!-- Release notes if available -->
              {#if $updateStore.status === 'available' && $updateStore.releaseNotes}
                <div class="divider"></div>
                <div class="update-notes-box">
                  <div class="update-notes-title">{$language === 'vi' ? 'Chi tiết bản phát hành:' : 'Release Notes:'}</div>
                  <pre class="update-notes-content">{$updateStore.releaseNotes}</pre>
                </div>
              {/if}
            </div>
          {/if}

          {#if error}
            <div class="error-row" role="alert">{error}</div>
          {/if}
        </div>
        {/key}
      </div>
    </div>

    <!-- ── Full-Width Bottom Action Footer (V-Notch Style) ──────────────────── -->
    <footer class="footer">
      <button type="button" class="btn-reset" on:click={resetToDefault} disabled={pickingFolder || isClosing}>
        {$language === 'vi' ? 'Đặt lại' : 'Reset'}
      </button>

      <div class="footer-end">
        <button
          type="button"
          class="btn-apply"
          class:btn-applied={appliedSuccess}
          on:click={handleApply}
          disabled={pickingFolder || isClosing}
        >
          {#if appliedSuccess}<CheckIcon size={16} weight="bold" aria-hidden="true" />{/if}
          {appliedSuccess ? ($language === 'vi' ? 'Đã áp dụng' : 'Applied') : ($language === 'vi' ? 'Áp dụng' : 'Apply')}
        </button>

        <button
          type="button"
          class="btn-save"
          on:click={handleSave}
          disabled={pickingFolder || isClosing}
        >
          {$language === 'vi' ? 'Lưu' : 'Save'}
        </button>
      </div>
    </footer>
  </div>
  </div>
</div>

<style>
  /* ── Interactive Element & Focus Reset ──────────────────────────────────────── */
  button, input, .nav-item {
    outline: none !important;
    -webkit-tap-highlight-color: transparent;
  }
  button:focus, button:focus-visible,
  input:focus, input:focus-visible,
  .nav-item:focus, .nav-item:focus-visible,
  .btn-reset:focus, .btn-reset:focus-visible,
  .btn-apply:focus, .btn-apply:focus-visible,
  .btn-save:focus, .btn-save:focus-visible,
  .btn-close:focus, .btn-close:focus-visible,
  .btn-social:focus, .btn-social:focus-visible,
  .btn-subtle:focus, .btn-subtle:focus-visible,
  .btn-clear-search:focus, .btn-clear-search:focus-visible {
    outline: none !important;
    box-shadow: none !important;
  }

  /* ── Backdrop ──────────────────────────────────────────────────────────────── */
  .backdrop {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    pointer-events: auto;
  }

  /* ── Shadow wrapper — uses GPU-composited filter so it never bleeds ────────── */
  .panel-shadow {
    border-radius: 18px;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.6);
    will-change: transform, opacity;
    flex-shrink: 0;
  }

  /* ── Panel Shell (V-Notch Modal Style) ─────────────────────────────────────── */
  .panel {
    width: min(800px, calc(100vw - 32px));
    height: min(540px, calc(100vh - 32px));
    display: flex;
    flex-direction: column;
    background: #000000;
    color: #e4e4e7;
    border-radius: 18px;
    border: 1px solid rgba(255, 255, 255, 0.09);
    /* No box-shadow — handled by .panel-shadow filter above */
    font-family: 'SF Pro Display', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    font-weight: 700;
    -webkit-font-smoothing: antialiased;
    overflow: hidden;
    outline: none !important;
    pointer-events: auto;
  }

  .shell {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }

  /* ── Left Sidebar (V-Notch Style) ─────────────────────────────────────────── */
  .sidebar {
    width: 205px;
    flex-shrink: 0;
    background: #060608;
    border-right: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    flex-direction: column;
    padding: 14px 10px 12px 12px;
  }

  .search-box {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 34px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 0 10px;
    margin-bottom: 12px;
    transition: border-color 0.16s ease, background 0.16s ease;
  }

  .search-box:focus-within {
    border-color: rgba(255, 255, 255, 0.22);
    background: rgba(255, 255, 255, 0.08);
  }

  .search-box :global(.search-icon) {
    color: #666666;
    flex-shrink: 0;
  }

  .search-input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    color: #ffffff;
    font-size: 12.5px;
    font-weight: 700;
    outline: none !important;
    font-family: inherit;
  }

  .search-input::placeholder {
    color: #555555;
    font-weight: 600;
  }

  .btn-clear-search {
    display: grid;
    place-items: center;
    min-width: 24px;
    min-height: 24px;
    background: transparent;
    border: 0;
    color: #888888;
    cursor: pointer;
    padding: 2px;
    font-size: 11px;
    line-height: 1;
  }
  .btn-clear-search:hover { color: #ffffff; }

  /* ── Nav List ─────────────────────────────────────────────────────────────── */
  .nav-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    font-size: 13px;
    font-weight: 700;
    color: #8c959e;
    background: transparent;
    border: 0;
    border-radius: 9px;
    cursor: pointer;
    text-align: left;
    outline: none !important;
    box-shadow: none !important;
    -webkit-tap-highlight-color: transparent;
    user-select: none;
    -webkit-user-select: none;
    transition: color 0.14s ease, background 0.14s ease;
  }

  .nav-item:focus,
  .nav-item:focus-visible {
    outline: none !important;
    box-shadow: none !important;
  }

  .nav-item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #ffffff;
  }

  .nav-item.nav-active {
    background: rgba(255, 255, 255, 0.1);
    color: #ffffff;
    font-weight: 700;
  }

  .nav-icon {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 18px;
    height: 18px;
  }

  .nav-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 700;
  }

  /* ── Sidebar Footer ───────────────────────────────────────────────────────── */
  .sidebar-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 10px;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
    flex-shrink: 0;
    margin-top: 6px;
  }

  .sidebar-socials {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .btn-social {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    background: transparent;
    border: 0;
    color: #8c959e;
    border-radius: 4px;
    cursor: pointer;
    transition: color 0.15s ease;
  }

  .btn-social:hover {
    color: #ffffff;
  }

  .build-tag {
    font-size: 11px;
    font-weight: 500;
    color: #444444;
  }

  /* ── Right Content Area ───────────────────────────────────────────────────── */
  .content-area {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: #000000;
  }

  .content-header {
    height: 52px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 22px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    flex-shrink: 0;
    cursor: default;
    user-select: none;
  }

  .header-left {
    display: flex;
    align-items: center;
  }

  .hero-title {
    font-size: 18px;
    font-weight: 700;
    color: #ffffff;
    letter-spacing: -0.01em;
    margin: 0;
  }

  .btn-close {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.06);
    color: #b5bcc4;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    cursor: pointer;
    font-size: 11px;
    line-height: 1;
    transition: background 0.15s ease, color 0.15s ease;
  }

  .btn-close:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
  }

  /* ── Content Scroll Pane ──────────────────────────────────────────────────── */
  .content-scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 14px 22px 20px;
    min-height: 0;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.12) transparent;
  }

  .content-scroll::-webkit-scrollbar {
    width: 5px;
  }

  .content-scroll::-webkit-scrollbar-track {
    background: transparent;
  }

  .content-scroll::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.14);
    border-radius: 999px;
  }

  /* ── Cards & Sections (V-Notch Style) ─────────────────────────────────────── */
  .section-title {
    font-size: 11px;
    font-weight: 700;
    color: #7c7c82;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-top: 18px;
    margin-bottom: 8px;
  }

  .card {
    background: #0b0b0e;
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 14px;
    padding: 14px 18px;
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .empty-search-card {
    align-items: center;
    justify-content: center;
    padding: 40px 20px;
    text-align: center;
    gap: 6px;
  }

  .empty-title {
    font-size: 13.5px;
    font-weight: 700;
    color: #d4d4d8;
  }

  .empty-sub {
    font-size: 11.5px;
    font-weight: 600;
    color: #71717a;
  }

  .divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.04);
    margin: 10px 0;
    flex-shrink: 0;
  }

  /* ── Setting Row ──────────────────────────────────────────────────── */
  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 40px;
  }

  .setting-row.vertical {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
  }

  .row-info {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .row-title {
    font-size: 13.5px;
    font-weight: 700;
    color: #e4e4e7;
    line-height: 1.35;
  }

  .row-desc {
    font-size: 11.5px;
    font-weight: 700;
    color: #8c959e;
    line-height: 1.4;
    margin-top: 2px;
  }

  /* ── iOS-style Toggle Switch (V-Notch Vibrant Green) ──────────────────────── */
  .switch {
    position: relative;
    display: inline-block;
    width: 42px;
    height: 24px;
    flex-shrink: 0;
    cursor: pointer;
  }

  .switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch .slider {
    position: absolute;
    inset: 0;
    background: #141418;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 999px;
    transition: background 0.2s ease, border-color 0.2s ease;
  }

  .switch input:disabled + .slider {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .switch input:focus-visible + .slider {
    outline: 2px solid #22c55e;
    outline-offset: 3px;
  }

  .switch .slider::before {
    content: '';
    position: absolute;
    left: 3px;
    top: 3px;
    width: 16px;
    height: 16px;
    background: #b0b0b0;
    border-radius: 50%;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
    transition: transform 0.2s cubic-bezier(0.34, 1.25, 0.64, 1), background 0.2s ease;
  }

  .switch input:checked + .slider {
    background: #22c55e;
    border-color: #22c55e;
  }

  .switch input:checked + .slider::before {
    background: #ffffff;
    transform: translateX(18px);
  }

  /* ── Segmented Control ─────────────────────────────────────────────────────── */
  .seg {
    display: flex;
    gap: 3px;
    background: rgba(255, 255, 255, 0.05);
    padding: 3px;
    border-radius: 9px;
    border: 1px solid rgba(255, 255, 255, 0.07);
    flex-shrink: 0;
  }

  .seg.seg-full {
    width: 100%;
  }
  .seg.seg-full .seg-btn {
    flex: 1;
  }

  .seg-btn {
    height: 28px;
    padding: 0 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    font-weight: 700;
    color: #8c959e;
    background: transparent;
    border: 0;
    border-radius: 7px;
    cursor: pointer;
    white-space: nowrap;
    transition: color 0.15s ease, background 0.15s ease;
  }

  .seg-btn:hover { color: #ffffff; }

  .seg-btn.seg-on {
    background: rgba(255, 255, 255, 0.14);
    color: #ffffff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
  }

  .seg-btn.seg-danger.seg-on {
    background: rgba(239, 68, 68, 0.25);
    color: #fca5a5;
  }

  /* ── Chips / Presets ──────────────────────────────────────────────────────── */
  .chips {
    display: flex;
    gap: 6px;
  }

  .chip {
    flex: 1;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    font-weight: 700;
    color: #8c959e;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .chip:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  .chip.chip-on {
    background: rgba(255, 255, 255, 0.14);
    border-color: rgba(255, 255, 255, 0.18);
    color: #ffffff;
    font-weight: 700;
  }

  /* ── Folder row ───────────────────────────────────────────────────────────── */
  .folder-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .folder-path {
    flex: 1;
    min-width: 0;
    height: 32px;
    display: flex;
    align-items: center;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 8px;
    padding: 0 10px;
    color: #8c959e;
    font-size: 11.5px;
    font-weight: 700;
  }

  .path-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .btn-subtle {
    height: 32px;
    padding: 0 14px;
    font-size: 12px;
    font-weight: 700;
    color: #c8c8c8;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }

  .btn-subtle:hover { background: rgba(255, 255, 255, 0.12); color: #ffffff; }

  /* ── Twin Number Inputs ───────────────────────────────────────────────────── */
  .twin-inputs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .input-cell {
    display: flex;
    align-items: center;
    height: 32px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 8px;
    overflow: hidden;
    transition: border-color 0.15s ease;
  }

  .input-cell:focus-within {
    border-color: rgba(255, 255, 255, 0.25);
  }

  .input-tag {
    padding: 0 8px;
    font-size: 10px;
    font-weight: 700;
    color: #666666;
    border-right: 1px solid rgba(255, 255, 255, 0.06);
    height: 100%;
    display: flex;
    align-items: center;
    user-select: none;
  }

  .num-input {
    flex: 1;
    height: 100%;
    font-size: 11.5px;
    font-weight: 700;
    color: #ffffff;
    background: transparent;
    border: 0;
    padding: 0 8px;
    outline: none;
    font-family: inherit;
  }

  /* ── Color Swatch Pill ────────────────────────────────────────────────────── */
  .swatch-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 999px;
    padding: 0 10px 0 5px;
    cursor: pointer;
    position: relative;
    flex-shrink: 0;
  }

  .swatch-dot {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.25);
    flex-shrink: 0;
  }

  .swatch-hex {
    font-size: 11px;
    font-weight: 700;
    color: #e4e4e7;
    letter-spacing: 0.03em;
  }

  .color-hidden {
    position: absolute;
    inset: 0;
    opacity: 0;
    width: 100%;
    height: 100%;
    cursor: pointer;
  }

  /* ── Badges ───────────────────────────────────────────────────────────────── */
  .badge-key {
    display: inline-flex;
    align-items: center;
    height: 24px;
    padding: 0 8px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    font-size: 11px;
    font-weight: 700;
    color: #c8c8c8;
  }

  /* ── Warning & Error ──────────────────────────────────────────────────────── */
  .error-row {
    font-size: 11px;
    font-weight: 700;
    color: #f87171;
    background: rgba(239, 68, 68, 0.08);
    border: 1px solid rgba(239, 68, 68, 0.18);
    border-radius: 8px;
    padding: 7px 11px;
    margin-top: 10px;
  }

  /* ── Full-Width Footer (V-Notch Style) ────────────────────────────────────── */
  .footer {
    height: 56px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 20px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    background: #060608;
    flex-shrink: 0;
  }

  .btn-reset {
    font-size: 13px;
    font-weight: 700;
    color: #8c959e;
    background: transparent;
    border: 0;
    cursor: pointer;
    padding: 6px 10px;
    border-radius: 8px;
    transition: color 0.15s ease;
  }
  .btn-reset:hover {
    color: #ffffff;
  }

  .footer-end {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .btn-apply {
    height: 36px;
    min-width: 90px;
    padding: 0 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 700;
    color: #c8c8c8;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 11px;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .btn-apply:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
  }
  .btn-apply:active { transform: scale(0.97); }

  .btn-apply.btn-applied {
    background: rgba(34, 197, 94, 0.18);
    border-color: rgba(34, 197, 94, 0.35);
    color: #4ade80;
  }

  .btn-save {
    height: 36px;
    min-width: 90px;
    padding: 0 22px;
    font-size: 13px;
    font-weight: 700;
    color: #000000;
    background: #ffffff;
    border: 0;
    border-radius: 11px;
    cursor: pointer;
    transition: background 0.15s ease, transform 0.12s ease;
  }

  .btn-save:hover {
    background: #e4e4e7;
    transform: translateY(-1px);
  }
  .btn-save:active { transform: scale(0.97) translateY(0); }

  .legal-links {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  /* ── Auto Update UI Styles ─────────────────────────────────────────────────── */
  .nav-update-badge {
    margin-left: auto;
    font-size: 9px;
    font-weight: 800;
    padding: 2px 6px;
    border-radius: 6px;
    background: #2563eb;
    color: #ffffff;
    letter-spacing: 0.5px;
    box-shadow: 0 0 10px rgba(37, 99, 235, 0.45);
    animation: pulse-badge 2s infinite ease-in-out;
  }

  @keyframes pulse-badge {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.8; transform: scale(1.05); }
  }

  .update-card-active {
    border-color: rgba(37, 99, 235, 0.35) !important;
    background: rgba(37, 99, 235, 0.04) !important;
  }

  .update-header-line {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .badge-update-pulse {
    font-size: 10px;
    font-weight: 800;
    padding: 2px 7px;
    border-radius: 6px;
    background: rgba(37, 99, 235, 0.2);
    border: 1px solid rgba(37, 99, 235, 0.5);
    color: #60a5fa;
    letter-spacing: 0.4px;
  }

  .update-error-text {
    color: #f87171;
  }

  .update-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .btn-update-primary {
    height: 32px;
    padding: 0 14px;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 700;
    color: #ffffff;
    background: #2563eb;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 9px;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(37, 99, 235, 0.35);
    transition: all 0.15s ease;
  }
  .btn-update-primary:hover:not(:disabled) {
    background: #1d4ed8;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(37, 99, 235, 0.5);
  }
  .btn-update-primary:active:not(:disabled) {
    transform: translateY(0) scale(0.97);
  }
  .btn-update-primary:disabled {
    opacity: 0.7;
    cursor: not-allowed;
  }

  .update-progress-container {
    width: 100%;
    height: 5px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 999px;
    overflow: hidden;
    margin-top: 10px;
  }

  .update-progress-bar {
    height: 100%;
    background: linear-gradient(90deg, #2563eb, #60a5fa);
    border-radius: 999px;
    transition: width 0.2s ease-out;
  }

  .update-notes-box {
    margin-top: 8px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 8px;
    max-height: 140px;
    overflow-y: auto;
  }

  .update-notes-title {
    font-size: 11px;
    font-weight: 700;
    color: #a1a1aa;
    margin-bottom: 6px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .update-notes-content {
    font-family: inherit;
    font-size: 12px;
    color: #d4d4d8;
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
    line-height: 1.5;
  }

  .spinner-inline {
    width: 12px;
    height: 12px;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    display: inline-block;
  }

  button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  button:focus-visible,
  input:focus-visible,
  .content-scroll :global([role="slider"]:focus-visible) {
    outline: 2px solid #ffffff !important;
    outline-offset: 2px;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
