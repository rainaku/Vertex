<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { t, language, syncNativeLanguage } from './lib/i18n';
  import RadialWheel from './lib/RadialWheel.svelte';
  import OptionsPanel from './lib/OptionsPanel.svelte';
  import { defaultOptions, loadOptions } from './lib/settings';
import { checkForUpdates } from './lib/updater';
  import {
    detectFile,
    getAvailableTargets,
    getBatchInfo,
    convertFile,
    cancelJob,
    listenProgress,
    sendDesktopNotification,
    setWindowPassthrough,
    hideWheelWindow,
    forceHideWheelWindow,
    setConvertingState,
    centerWindow,
    prepareSettingsWindow,
    restoreWheelWindow,
    isTauri,
  } from './lib/tauri-bridge';
  import { hitTest } from './lib/geometry';
  import type { TargetFormatInfo, Options, FormatInfo } from './lib/types';

  // ─── State ───────────────────────────────────────────────────────────────────
  let formats: TargetFormatInfo[] = [];
  let allAvailableFormats: TargetFormatInfo[] = [];
  let activeIndex: number = -1;
  let convertingIndex: number = -1;
  let progress: number = 0;
  let conversionError = '';
  let status: 'idle' | 'converting' | 'done' | 'error' = 'idle';

  let currentFile: FormatInfo | null = null;
  let currentFilePaths: string[] = [];
  let sourceFormatLabel: string | null = null;

  // Whether wheel is currently visible
  let wheelVisible: boolean = false;
  let wheelClosing = false;
  let wheelElement: HTMLDivElement;
  let visibilityRevision = 0;
  let nativeGeneration = 0;
  let cancelCloseWait: (() => void) | null = null;

  // Whether a file is being dragged over our window
  let isDragging: boolean = false;

  // Pagination
  let page: number = 0;
  const PETALS_PER_PAGE = 7;

  // Options
  let options: Options = defaultOptions();
  let settingsVisible = false;
  let optionsPanelRef: OptionsPanel | null = null;
  let unlistenSettings: (() => void) | null = null;
  let unlistenUpdateAvailable: (() => void) | null = null;

  async function openSettings() {
    if (status === 'converting') return;
    clearDragDwell();
    isDragging = false;
    wheelVisible = false;
    wheelClosing = false;
    await prepareSettingsWindow();
    await setWindowPassthrough(false);
    settingsVisible = true;
  }

  async function closeSettings() {
    settingsVisible = false;
    wheelVisible = false;
    wheelClosing = false;
    isDragging = false;
    clearDragDwell();
    await setWindowPassthrough(true);
    await forceHideWheelWindow();
    await restoreWheelWindow();
  }

  let unlistenProgress: (() => void) | null = null;
  let unlistenDragDrop: (() => void) | null = null;
  let unlistenDragStart: (() => void) | null = null;
  let unlistenDragEnd: (() => void) | null = null;
  let unlistenDragCancel: (() => void) | null = null;
  let unlistenWheelShown: (() => void) | null = null;
  let unlistenCloseRequested: (() => void) | null = null;

  // ─── Helpers ─────────────────────────────────────────────────────────────────

  function orderTargets(targets: TargetFormatInfo[], files: FormatInfo[]) {
    const source = files.length === 1 ? files[0].format : null;
    return [...targets]
      .sort((a, b) => Number(b.format === source) - Number(a.format === source));
  }

  function updatePagedFormats() {
    if (allAvailableFormats.length <= 8) {
      formats = allAvailableFormats;
    } else {
      const start = page * PETALS_PER_PAGE;
      const end = start + PETALS_PER_PAGE;
      formats = allAvailableFormats.slice(start, end);
    }
  }

  $: hasMorePages = allAvailableFormats.length > 8;
  $: totalPages = Math.ceil(allAvailableFormats.length / PETALS_PER_PAGE) || 1;

  let dragDwellTimer: any = null;
  let dragDwellCooldown = false;

  function triggerDragDwell() {
    if (status === 'converting' || status === 'done') return;
    if (dragDwellCooldown || !hasMorePages) return;
    if (!dragDwellTimer) {
      dragDwellTimer = setTimeout(() => {
        handleNextPage();
        dragDwellTimer = null;
        dragDwellCooldown = true;
        setTimeout(() => {
          dragDwellCooldown = false;
        }, 750);
      }, 260);
    }
  }

  function clearDragDwell() {
    if (dragDwellTimer) {
      clearTimeout(dragDwellTimer);
      dragDwellTimer = null;
    }
  }

  function handleNextPage() {
    if (status === 'converting' || status === 'done') return;
    page = (page + 1) % totalPages;
    updatePagedFormats();
  }

  // ─── Wheel visibility ────────────────────────────────────────────────────────

  async function showWheel() {
    if (settingsVisible) return;
    cancelClose();
    if (wheelVisible) return;
    wheelVisible = true;
    // Disable passthrough so user can interact with the wheel
    await setWindowPassthrough(false);
  }

  function cancelClose() {
    visibilityRevision++;
    cancelCloseWait?.();
    cancelCloseWait = null;
    wheelClosing = false;
  }

  async function hideWheel() {
    if (settingsVisible) return;
    if (wheelClosing) return;
    const revision = ++visibilityRevision;
    const generation = nativeGeneration;
    wheelClosing = true;
    wheelVisible = false;
    activeIndex = -1;
    isDragging = false;
    clearDragDwell();
    // Let Svelte apply the closing styles before collecting their transitions.
    await tick();
    if (revision !== visibilityRevision) return;
    const transitions = wheelElement?.getAnimations() ?? [];
    await new Promise<void>((resolve) => {
      // Also finish if a hidden/throttled webview never delivers completion.
      const timeout = window.setTimeout(finish, 300);
      function finish() {
        window.clearTimeout(timeout);
        resolve();
      }
      cancelCloseWait = finish;
      Promise.allSettled(transitions.map((animation) => animation.finished)).then(finish);
    });
    if (revision !== visibilityRevision) return;
    cancelCloseWait = null;
    try {
      // Keep the native surface alive for the entire CSS exit, then hide it.
      // Rust rejects this close if another drag has already reopened the window.
      await hideWheelWindow(generation);
    } catch (error) {
      console.warn('Failed to hide wheel window:', error);
    } finally {
      if (revision === visibilityRevision) wheelClosing = false;
    }
  }

  // ─── File handling ───────────────────────────────────────────────────────────

  async function handleFileLoaded(paths: string[]) {
    if (paths.length === 0) return;
    conversionError = '';
    currentFilePaths = paths;

    if (paths.length === 1) {
      try {
        const info = await detectFile(paths[0]);
        currentFile = info;
        sourceFormatLabel = info.label;
        const targets = await getAvailableTargets(info.format);
        allAvailableFormats = orderTargets(targets, [info]);
        page = 0;
        updatePagedFormats();
      } catch (e) {
        console.error('File detection failed:', e);
      }
    } else {
      try {
        const batch = await getBatchInfo(paths);
        currentFile = batch.files[0] || null;
        sourceFormatLabel = `BATCH (${paths.length})`;
        allAvailableFormats = orderTargets(batch.common_targets, batch.files);
        page = 0;
        updatePagedFormats();
      } catch (e) {
        console.error('Batch detection failed:', e);
      }
    }
  }

  // ─── Conversion ──────────────────────────────────────────────────────────────

  async function startConversion(targetFmt: TargetFormatInfo, index: number) {
    if (!currentFilePaths.length || settingsVisible || status === 'converting') return;
    conversionError = '';
    const conversionOptions = structuredClone(options);

    clearDragDwell();
    convertingIndex = index;
    activeIndex = index;
    status = 'converting';
    progress = 0.05;
    cancelClose();
    wheelVisible = true;
    const jobId = `job_${Date.now()}`;
    activeJobId = jobId;
    stopping = false;
    const firstPath = currentFilePaths[0];

    try {
      const res = await convertFile(jobId, firstPath, targetFmt.format, conversionOptions);
      if (res.success) {
        status = 'done';
        progress = 1.0;

        setTimeout(async () => {
          if (status === 'done') {
            status = 'idle';
            convertingIndex = -1;
            await setConvertingState(false);
            await hideWheel();
          }
        }, 1200);
      } else if (res.cancelled) {
        status = 'idle';
        convertingIndex = -1;
        activeIndex = -1;
        progress = 0;
        await hideWheel();
      } else {
        conversionError = res.error || ($language === 'vi' ? 'Chuyển đổi thất bại.' : 'Conversion failed.');
        status = 'error';
        setTimeout(async () => {
          status = 'idle';
          convertingIndex = -1;
          await setConvertingState(false);
        }, 2000);
      }
    } catch (e: any) {
      conversionError = String(e);
      status = 'error';
      setTimeout(async () => {
        status = 'idle';
        convertingIndex = -1;
        await setConvertingState(false);
      }, 2000);
    } finally {
      activeJobId = null;
      stopping = false;
    }
  }

  let activeJobId: string | null = null;
  let stopping = false;
  async function stopConversion() {
    if (!activeJobId || stopping) return;
    stopping = true;
    try { await cancelJob(activeJobId); }
    catch (error) { stopping = false; console.warn('Cancel failed:', error); }
  }

  // ─── Keyboard ────────────────────────────────────────────────────────────────

  function handleKeyDown(e: KeyboardEvent) {
    if (settingsVisible) return;
    if (e.key === 'Escape') {
      if (activeJobId) { void stopConversion(); return; }
      hideWheel();
    }
  }

  // ─── Lifecycle ───────────────────────────────────────────────────────────────

  onMount(async () => {
    options = loadOptions();
    void syncNativeLanguage($language).catch(error => console.warn('Language sync failed:', error));
    window.addEventListener('keydown', handleKeyDown);

    // Subscribe to conversion progress
    unlistenProgress = await listenProgress((payload) => {
      if (payload.job_id !== activeJobId) return;
      if (stopping) {
        // Also covers a stop click before the conversion command registered
        // its token. Progress is emitted only after registration.
        void cancelJob(payload.job_id).catch(error => console.warn('Cancel failed:', error));
        return;
      }
      progress = payload.progress;
    });

    // Tauri drag-and-drop & global shortcut events
    if (isTauri) {
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        const { listen } = await import('@tauri-apps/api/event');
        const webview = getCurrentWebview();
        unlistenSettings = await listen('open_advanced_settings', () => { void openSettings(); });

        unlistenWheelShown = await listen<{ generation: number; focus: boolean }>('wheel_shown', (event) => {
          nativeGeneration = event.payload.generation;
          cancelClose();
          if (event.payload.focus && !settingsVisible) void showWheel();
        });
        unlistenCloseRequested = await listen('wheel_close_requested', () => {
          if (settingsVisible) {
            if (optionsPanelRef) {
              optionsPanelRef.requestClose();
            } else {
              void closeSettings();
            }
          } else {
            void hideWheel();
          }
        });

        // 1. Listen for global Shift+Drag events from Rust background thread
        unlistenDragStart = await listen('shift_drag_start', () => {
          if (settingsVisible) return;
          isDragging = true;
        });

        unlistenDragEnd = await listen('shift_drag_end', () => {
          if (settingsVisible) return;
          if (status !== 'converting') {
            hideWheel();
          }
        });

        unlistenDragCancel = await listen('drag_cancelled', () => {
          if (settingsVisible) return;
          if (status !== 'converting') {
            hideWheel();
          }
        });

        // 2. Tauri webview drag-and-drop events (triggered when files hover over webview)
        unlistenDragDrop = await webview.onDragDropEvent(async (event) => {
          if (settingsVisible) return;
          if (status === 'converting' || status === 'done') {
            clearDragDwell();
            return;
          }

          const dpr = window.devicePixelRatio || 1;

          if (event.payload.type === 'enter') {
            const paths = event.payload.paths;
            isDragging = true;
            await handleFileLoaded(paths);
            if (!isDragging || wheelClosing || settingsVisible) return;
            await showWheel();
          } else if (event.payload.type === 'over') {
            if (wheelClosing) return;
            if (!wheelVisible) {
              await showWheel();
            }
            const pos = event.payload.position;
            // Convert physical coordinates to CSS logical coordinates
            const cssX = pos.x / dpr;
            const cssY = pos.y / dpr;
            // Scale = 0.55 for compact wheel (286px diameter)
            const SCALE = 0.55;
            const centerX = window.innerWidth / 2;
            const centerY = window.innerHeight / 2;
            const localX = (cssX - centerX) / SCALE + 260;
            const localY = (cssY - centerY) / SCALE + 260;
            const hit = hitTest(localX, localY, 260, 260, 70, 244, 8);
            activeIndex = hit.type === 'petal' ? hit.index : -1;

            if (hit.type === 'petal' && hit.index === 7 && hasMorePages) {
              triggerDragDwell();
            } else {
              clearDragDwell();
            }
          } else if (event.payload.type === 'drop') {
            const paths = event.payload.paths;
            const pos = event.payload.position;
            isDragging = false;
            clearDragDwell();

            if (wheelVisible) {
              const cssX = pos.x / dpr;
              const cssY = pos.y / dpr;
              const SCALE = 0.55;
              const centerX = window.innerWidth / 2;
              const centerY = window.innerHeight / 2;
              const localX = (cssX - centerX) / SCALE + 260;
              const localY = (cssY - centerY) / SCALE + 260;
              const hit = hitTest(localX, localY, 260, 260, 70, 244, 8);

              // Dropping directly on the "..." petal flips page and stays open
              if (hit.type === 'petal' && hit.index === 7 && hasMorePages) {
                handleNextPage();
                return;
              }

              if (hit.type === 'petal' && hit.index >= 0 && hit.index < formats.length) {
                const target = formats[hit.index];
                if (target && target.available) {
                  conversionError = '';
    currentFilePaths = paths;
                  startConversion(target, hit.index);
                  return;
                }
              }
            }
            hideWheel();
          } else if (event.payload.type === 'leave') {
            clearDragDwell();
            activeIndex = -1;
            // Do not hide wheel on leave: spurious leave events fire when cursor moves between DOM elements.
            // The wheel closes when the user drops/releases the mouse button or presses Esc.
          }
        });
        unlistenUpdateAvailable = await listen('update_available', () => {
          void checkForUpdates(false);
        });
      } catch (err) {
        console.warn('Failed to register Tauri event handlers:', err);
      }
    }

    // Auto-check for updates 10 seconds after startup in background
    setTimeout(() => {
      void checkForUpdates(false);
    }, 10000);
  });

  onDestroy(() => {
    if (unlistenSettings) unlistenSettings();
    cancelClose();
    window.removeEventListener('keydown', handleKeyDown);
    if (unlistenProgress) unlistenProgress();
    if (unlistenDragDrop) unlistenDragDrop();
    if (unlistenDragStart) unlistenDragStart();
    if (unlistenDragEnd) unlistenDragEnd();
    if (unlistenDragCancel) unlistenDragCancel();
    if (unlistenWheelShown) unlistenWheelShown();
    if (unlistenCloseRequested) unlistenCloseRequested();
    if (unlistenUpdateAvailable) unlistenUpdateAvailable();
  });
</script>

<!-- 
  Ghost overlay: the entire viewport is transparent.
  The wheel fades in/out via CSS when wheelVisible changes.
-->
<main class="ghost-overlay">
  <!-- Radial Wheel — shown only when wheelVisible -->
  <div bind:this={wheelElement} class="wheel-wrapper" class:visible={wheelVisible && !settingsVisible} class:closing={wheelClosing}>
    <RadialWheel
      {formats}
      {conversionError}
      {activeIndex}
      {convertingIndex}
      {progress}
      {status}
      sourceFormat={currentFilePaths.length > 1 ? `${$t("Tệp")} (${currentFilePaths.length})` : sourceFormatLabel}
      {hasMorePages}
      {page}
      {totalPages}
      onSelectFormat={(fmt) => startConversion(fmt, activeIndex)}
      onHoverChange={(idx) => activeIndex = idx}
      onNextPage={handleNextPage}
      {stopping}
      onCenterClick={() => { if (activeJobId) void stopConversion(); else void openSettings(); }}
    />
  </div>

  <!-- Hint pill — shown only when dragging without Shift -->
  {#if isDragging && !wheelVisible}
    <div class="ctrl-hint">
      <span class="ctrl-key">Shift</span>
      <span>{$t("+ thả để chuyển đổi")}</span>
    </div>
  {/if}

</main>

{#if settingsVisible}
  <OptionsPanel
    bind:this={optionsPanelRef}
    {options}
    onClose={closeSettings}
    onSave={(value) => {
      options = value;
      void closeSettings();
    }}
  />
{/if}

<style>
  /* The entire window is a fully transparent passthrough overlay */
  .ghost-overlay {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    background: transparent;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none; /* JS-side passthrough; OS passthrough via set_ignore_cursor_events */
    overflow: hidden;
  }

  /* Wheel wrapper: compact size (scale 0.55), fades & pops in */
  .wheel-wrapper {
    pointer-events: none;
    opacity: 0;
    transform: scale(0.35);
    transform-origin: center center;
    transition:
      opacity 0.18s cubic-bezier(0.16, 1, 0.3, 1),
      transform 0.22s cubic-bezier(0.16, 1, 0.3, 1);
    will-change: opacity, transform;
  }

  .wheel-wrapper.visible {
    pointer-events: all;
    opacity: 1;
    transform: scale(0.55);
  }

  .wheel-wrapper.closing {
    pointer-events: none;
    opacity: 0;
    transform: scale(0.43);
    transition:
      opacity 0.18s cubic-bezier(0.4, 0, 1, 1),
      transform 0.2s cubic-bezier(0.4, 0, 1, 1);
  }

  /* Small hint pill shown during drag without Shift held */
  .ctrl-hint {
    position: fixed;
    bottom: 40px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 18px;
    background: #000000;
    border: none;
    border-radius: 9999px;
    color: #ffffff;
    font-family: var(--font-family);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.85);
    animation: hintSlideUp 0.25s cubic-bezier(0.16, 1, 0.3, 1);
    pointer-events: none;
  }

  .ctrl-key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: #ffffff;
    border: none;
    border-radius: 6px;
    padding: 2px 8px;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.5px;
    color: #000000;
  }

  @keyframes hintSlideUp {
    from {
      opacity: 0;
      transform: translateX(-50%) translateY(12px);
    }
    to {
      opacity: 1;
      transform: translateX(-50%) translateY(0);
    }
  }
</style>
