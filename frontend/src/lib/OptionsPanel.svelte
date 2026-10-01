<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { t, language, setLanguage } from './i18n';
  import type { Options } from './types';
  import { defaultOptions, normalizeOptions, SETTINGS_KEY } from './settings';
  import { isTauri } from './tauri-bridge';
  import ElasticSlider from './ElasticSlider.svelte';
  import { gsap } from 'gsap';

  export let options: Options;
  export let onSave: (value: Options) => void;
  export let onClose: () => void;

  let draft = normalizeOptions(structuredClone(options));
  let error = '';
  let pickingFolder = false;
  let isClosing = false;
  let appliedSuccess = false;
  let appliedTimer: any = null;
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
  let isTabSwitching = false;

  async function selectTab(id: TabKey) {
    if (id === activeTab || isTabSwitching) return;
    isTabSwitching = true;
    // Animate out current pane
    if (paneEl) {
      await gsap.to(paneEl, { opacity: 0, x: -10, duration: 0.12, ease: 'power2.in' });
    }
    activeTab = id;
    searchQuery = '';
    isTabSwitching = false;
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
      destroy() { paneEl = null; }
    };
  }

  export function requestClose(callback?: () => void) {
    if (isClosing) return;
    isClosing = true;

    const tl = gsap.timeline({
      onComplete: () => {
        if (callback) callback();
        else onClose();
      }
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

  function handleApply() {
    if (isClosing || pickingFolder) return;
    const value = normalizeOptions(draft);
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(value));
      onSave(value);
      appliedSuccess = true;
      if (appliedTimer) clearTimeout(appliedTimer);
      appliedTimer = setTimeout(() => {
        appliedSuccess = false;
      }, 1600);
    } catch {
      error = $t('Không thể lưu cài đặt trên thiết bị này.');
    }
  }

  function handleSave() {
    if (isClosing || pickingFolder) return;
    const value = normalizeOptions(draft);
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(value));
      requestClose(() => {
        onSave(value);
      });
    } catch {
      error = $t('Không thể lưu cài đặt trên thiết bị này.');
    }
  }

  function resetToDefault() {
    draft = defaultOptions();
  }

  // ── Clean Window Dragging ───────────────────────────────────────────────────

  async function handleHeaderPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    // Strictly prevent dragging when clicking interactive controls or scrolling
    if (target.closest('button, input, select, textarea, a, .btn-close, .search-box, .switch, .seg, .slider-root, .chip, .btn-subtle, .input-cell, .num-input, .swatch-pill, .content-scroll')) return;

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
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleGlobalKeyDown, true);
    if (appliedTimer) clearTimeout(appliedTimer);
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

  function matchText(text: string): boolean {
    return text.toLowerCase().includes(lowerSearch);
  }

  $: hasMatchLanguage = matchText('ngôn ngữ') || matchText('language') || matchText('tiếng việt') || matchText('english');
  $: hasMatchQuality = matchText('chất lượng') || matchText('quality') || matchText('jpg') || matchText('avif') || matchText('nén');
  $: hasMatchFolder = matchText('thư mục') || matchText('folder') || matchText('output') || matchText('đầu ra') || matchText('đích');
  $: hasMatchCollision = matchText('trùng') || matchText('tên tệp') || matchText('collision') || matchText('ghi đè') || matchText('overwrite');
  $: hasMatchMetadata = matchText('exif') || matchText('siêu dữ liệu') || matchText('metadata') || matchText('gps') || matchText('riêng tư');
  $: hasMatchBgColor = matchText('màu nền') || matchText('nền') || matchText('background') || matchText('color');
  $: hasMatchDimensions = matchText('kích thước') || matchText('rộng') || matchText('cao') || matchText('width') || matchText('height') || matchText('dimension');
  $: hasMatchVideoCodec = matchText('video') || matchText('codec') || matchText('h264') || matchText('h265') || matchText('mã hóa');
  $: hasMatchVideoCrf = matchText('crf') || matchText('nét') || matchText('dung lượng video');
  $: hasMatchAvifSpeed = matchText('avif') || matchText('tốc độ') || matchText('speed');
  $: hasMatchGifAlpha = matchText('gif') || matchText('alpha') || matchText('trong suốt') || matchText('threshold');
  $: hasMatchDpi = matchText('dpi') || matchText('pdf') || matchText('tài liệu') || matchText('phân giải');
  $: hasMatchVideoPreset = matchText('preset') || matchText('cpu') || matchText('fast') || matchText('slow');
  $: hasMatchShortcuts = matchText('phím tắt') || matchText('shortcut') || matchText('shift') || matchText('drag') || matchText('esc');

  $: hasAnySearchResult =
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
    on:pointerdown={handleHeaderPointerDown}
  >
    <!-- ── Shell Layout (Sidebar + Main Pane) ────────────────────────────────── -->
    <div class="shell">
      <!-- ── Left Sidebar (V-Notch Style) ───────────────────────────────────── -->
      <aside class="sidebar">
        <!-- Search Box -->
        <div class="search-box">
          <svg class="search-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="8"></circle>
            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
          </svg>
          <input
            type="text"
            class="search-input"
            placeholder={$language === 'vi' ? 'Tìm kiếm cài đặt…' : 'Search settings…'}
            bind:value={searchQuery}
          />
          {#if searchQuery}
            <button type="button" class="btn-clear-search" on:click={() => searchQuery = ''} aria-label="Clear">
              ✕
            </button>
          {/if}
        </div>

        <!-- Navigation items list -->
        <nav class="nav-list" aria-label="Settings Categories">
          {#if isSearching}
            <div class="nav-item nav-active">
              <span class="nav-icon">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                </svg>
              </span>
              <span class="nav-label">{$language === 'vi' ? 'Kết quả tìm kiếm' : 'Search results'}</span>
            </div>
          {:else}
            {#each TABS as tab}
              <button
                type="button"
                class="nav-item"
                class:nav-active={activeTab === tab.id}
                on:click={() => selectTab(tab.id)}
              >
                <span class="nav-icon">
                  {#if tab.icon === 'general'}
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/><line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/><line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/><line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="17" y1="16" x2="23" y2="16"/>
                    </svg>
                  {:else if tab.icon === 'image'}
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <rect x="3" y="3" width="18" height="18" rx="2.5"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/>
                    </svg>
                  {:else if tab.icon === 'video'}
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2"/>
                    </svg>
                  {:else if tab.icon === 'advanced'}
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
                    </svg>
                  {:else if tab.icon === 'shortcuts'}
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <rect x="2" y="4" width="20" height="16" rx="3"/><path d="M6 8h.01M10 8h.01M14 8h.01M18 8h.01M8 12h.01M12 12h.01M16 12h.01M7 16h10"/>
                    </svg>
                  {:else}
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/>
                    </svg>
                  {/if}
                </span>
                <span class="nav-label">{$language === 'vi' ? tab.labelVi : tab.labelEn}</span>
              </button>
            {/each}
          {/if}
        </nav>

        <!-- Sidebar Footer (V-Notch Style) -->
        <div class="sidebar-footer">
          <div class="sidebar-socials">
            <button type="button" class="btn-social" title="Vertex Website" on:click={() => { if (typeof window !== 'undefined') window.open?.('https://vertex.app', '_blank'); }}>
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                <circle cx="12" cy="12" r="10"/>
                <line x1="2" y1="12" x2="22" y2="12"/>
                <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
              </svg>
            </button>
            <button type="button" class="btn-social" title="GitHub" on:click={() => { if (typeof window !== 'undefined') window.open?.('https://github.com', '_blank'); }}>
              <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
              </svg>
            </button>
          </div>
          <span class="build-tag">Build 1.0.0</span>
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
            ✕
          </button>
        </header>

        <!-- Scrollable settings list -->
        {#key `${activeTab}::${isSearching}`}
        <div class="content-scroll" use:gsapPane>
          {#if isSearching}
            <!-- ── Search Results Mode ──────────────────────────────────────── -->
            <div class="section-title">{$language === 'vi' ? 'KẾT QUẢ TÌM KIẾM' : 'SEARCH RESULTS'}</div>
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
                      <button type="button" class="btn-subtle" on:click={chooseFolder}>{$language === 'vi' ? 'Duyệt…' : 'Browse…'}</button>
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
            {:else}
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
                    <button type="button" class="btn-subtle" on:click={() => draft.output_dir = undefined} title={$language === 'vi' ? 'Đặt lại mặc định' : 'Reset to default'}>
                      ✕
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
            </div>

          {:else if activeTab === 'shortcuts'}
            <!-- ── TAB: SHORTCUTS & GESTURES ────────────────────────────────── -->
            <div class="section-title">{$language === 'vi' ? 'THAO TÁC CỬ CHỈ' : 'GESTURES & SHORTCUTS'}</div>
            <div class="card">
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
            </div>

          {:else if activeTab === 'about'}
            <!-- ── TAB: ABOUT ───────────────────────────────────────────────── -->
            <div class="card">
              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">Vertex Converter</span>
                  <span class="row-desc">{$language === 'vi' ? 'Bộ công cụ chuyển đổi tệp đa phương tiện tức thì.' : 'Instant radial file conversion toolkit.'}</span>
                </div>
                <span class="badge-key">v0.1.0</span>
              </div>
              <div class="divider"></div>
              <div class="setting-row">
                <div class="row-info">
                  <span class="row-title">{$language === 'vi' ? 'Kiến trúc lõi' : 'Core Architecture'}</span>
                  <span class="row-desc">Rust Core Engine + Tauri 2 + Svelte 5 + GSAP</span>
                </div>
                <span class="badge-key">Rust + Tauri</span>
              </div>
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
      <button type="button" class="btn-reset" on:click={resetToDefault}>
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
          {appliedSuccess ? ($language === 'vi' ? '✓ Đã áp dụng' : '✓ Applied') : ($language === 'vi' ? 'Áp dụng' : 'Apply')}
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
    /* drop-shadow is composited at GPU layer: doesn't leak through transparent bg */
    filter:
      drop-shadow(0 1px 2px rgba(0, 0, 0, 0.55))
      drop-shadow(0 6px 18px rgba(0, 0, 0, 0.55))
      drop-shadow(0 16px 40px rgba(0, 0, 0, 0.6));
    will-change: transform, opacity;
    flex-shrink: 0;
  }

  /* ── Panel Shell (V-Notch Modal Style) ─────────────────────────────────────── */
  .panel {
    width: 800px;
    height: 540px;
    display: flex;
    flex-direction: column;
    background: #000000;
    color: #e4e4e7;
    border-radius: 18px;
    border: 1px solid rgba(255, 255, 255, 0.09);
    /* No box-shadow — handled by .panel-shadow filter above */
    font-family: 'SF Pro Display', 'SF Pro Bold', 'SF Pro', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
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

  .search-icon {
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
    width: 16px;
    height: 16px;
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
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    background: transparent;
    border: 0;
    color: #666666;
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
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.06);
    color: #888888;
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
    opacity: 0;
    width: 0;
    height: 0;
    pointer-events: none;
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
</style>
