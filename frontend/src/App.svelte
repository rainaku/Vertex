<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { t, language, syncNativeLanguage } from './lib/i18n';
  import RadialWheel from './lib/RadialWheel.svelte';
  import type OptionsPanel from './lib/OptionsPanel.svelte';
  import { defaultOptions, loadOptions } from './lib/settings';
  import { playSound } from './lib/sound';
  import './lib/theme';
  import { runConversionBatch } from './lib/conversion-batch';
  import { checkForUpdates } from './lib/updater';
  import {
    detectFile,
    getAvailableTargets,
    getBatchInfo,
    convertFile,
    cancelJob,
    listenProgress,
    setWindowPassthrough,
    hideWheelWindow,
    setConvertingState,
    prepareSettingsWindow,
    restoreWheelWindow,
    isTauri,
  } from './lib/tauri-bridge';
  import { hitTestClient } from './lib/geometry';
  import type { TargetFormatInfo, Options, FormatInfo } from './lib/types';

  // ─── State ───────────────────────────────────────────────────────────────────
  let destroyed = false;
  let statusTimer: ReturnType<typeof setTimeout>;
  let updateTimer: ReturnType<typeof setTimeout>;
  let dragCooldownTimer: ReturnType<typeof setTimeout>;
  let batchCompleted = 0;
  let batchTotal = 1;
  let fileRevision = 0;
  const ownedListeners = new Set<() => void>();
  function keepListener(cleanup: () => void) {
    if (destroyed) cleanup();
    else ownedListeners.add(cleanup);
    return () => { if (ownedListeners.delete(cleanup)) cleanup(); };
  }
  const scopedProgress: typeof listenProgress = (callback) => listenProgress(callback).then(keepListener);
  let formats: TargetFormatInfo[] = [];
  let allAvailableFormats: TargetFormatInfo[] = [];
  let activeIndex: number = -1;
  let convertingIndex: number = -1;
  let progress: number = 0;
  let conversionError = '';
  let status: 'idle' | 'converting' | 'done' | 'error' = 'idle';

  let currentFile: FormatInfo | null = null;
  let currentFilePaths: string[] = [];
  let sourceFormat: string | null = null;
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
  let dragInsideWebview = false;
  let dragReleased = false;
  let dragCancelled = false;
  let dropAccepted = false;
  let dragRevision = 0;
  let loadingFiles: { paths: string[]; ready: Promise<boolean> } | null = null;

  function isCurrentDrag(revision: number) {
    return revision === dragRevision && !dragCancelled && !destroyed && !settingsVisible;
  }

  // Pagination
  let page: number = 0;
  const PETALS_PER_PAGE = 7;

  // Options
  let options: Options = defaultOptions();
  let settingsVisible = false;
  let optionsPanelRef: OptionsPanel | null = null;

  async function openSettings() {
    if (status === 'converting' || settingsVisible) return;
    clearDragDwell();
    cancelClose();
    cachedWheelRect = null;
    dragRevision++;
    dragCancelled = true;
    dragInsideWebview = false;
    isDragging = false;
    wheelVisible = false;
    wheelClosing = false;
    settingsVisible = true;
    await prepareSettingsWindow();
  }

  async function closeSettings() {
    settingsVisible = false;
    wheelVisible = false;
    wheelClosing = false;
    isDragging = false;
    clearDragDwell();
    await restoreWheelWindow();
  }

  // ─── Helpers ─────────────────────────────────────────────────────────────────

  const OPEN_WHEEL_SIZE = 520 * 0.55;
  let cachedWheelRect: Pick<DOMRect, 'left' | 'top' | 'width' | 'height'> | null = null;

  function getOpenWheelRect(): Pick<DOMRect, 'left' | 'top' | 'width' | 'height'> {
    if (wheelVisible && cachedWheelRect) return cachedWheelRect;
    const rect = wheelElement?.getBoundingClientRect();
    if (wheelVisible && rect && rect.width >= OPEN_WHEEL_SIZE && rect.height >= OPEN_WHEEL_SIZE) {
      cachedWheelRect = rect;
      return rect;
    }
    const cx = rect && rect.width > 0 ? rect.left + rect.width / 2 : window.innerWidth / 2;
    const cy = rect && rect.height > 0 ? rect.top + rect.height / 2 : window.innerHeight / 2;
    return {
      left: cx - OPEN_WHEEL_SIZE / 2,
      top: cy - OPEN_WHEEL_SIZE / 2,
      width: OPEN_WHEEL_SIZE,
      height: OPEN_WHEEL_SIZE,
    };
  }

  function getCommonSourceFormat(files: FormatInfo[]): string | null {
    if (files.length === 0) return null;
    const first = files[0].format;
    return files.every((file) => file.format === first) ? first : null;
  }

  function orderTargets(targets: TargetFormatInfo[], files: FormatInfo[]) {
    const source = getCommonSourceFormat(files);
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

  let dragDwellTimer: ReturnType<typeof setTimeout> | null = null;
  let dragDwellCooldown = false;

  function triggerDragDwell() {
    if (status === 'converting' || status === 'done') return;
    if (dragDwellCooldown || !hasMorePages) return;
    if (!dragDwellTimer) {
      dragDwellTimer = setTimeout(() => {
        handleNextPage();
        dragDwellTimer = null;
        dragDwellCooldown = true;
        dragCooldownTimer = setTimeout(() => {
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
    playSound('page_turn');
  }

  // ─── Wheel visibility ────────────────────────────────────────────────────────

  async function showWheel() {
    if (settingsVisible) return;
    cancelClose();
    if (wheelVisible) return;
    cachedWheelRect = null;
    wheelVisible = true;
    playSound('wheel_open');
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
    const wasVisible = wheelVisible;
    cachedWheelRect = null;
    dragRevision++;
    dragCancelled = true;
    dragInsideWebview = false;
    fileRevision++;
    const revision = ++visibilityRevision;
    const generation = nativeGeneration;
    wheelClosing = true;
    wheelVisible = false;
    activeIndex = -1;
    isDragging = false;
    clearDragDwell();
    if (wasVisible && status === 'idle') playSound('wheel_close');
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

  async function handleFileLoaded(paths: string[]): Promise<boolean> {
    if (!paths.length || status === 'converting') return false;
    const revision = ++fileRevision;
    currentFilePaths = [];
    currentFile = null;
    sourceFormat = null;
    sourceFormatLabel = null;
    activeIndex = -1;
    allAvailableFormats = [];
    formats = [];
    conversionError = '';
    try {
      const batch = paths.length === 1
        ? await (async () => {
            const file = await detectFile(paths[0]);
            return { files: [file], common_targets: await getAvailableTargets(file.format) };
          })()
        : await getBatchInfo(paths);
      if (destroyed || revision !== fileRevision) return false;
      currentFilePaths = [...paths];
      status = 'idle';
      convertingIndex = -1;
      currentFile = batch.files[0] ?? null;
      sourceFormat = getCommonSourceFormat(batch.files);
      sourceFormatLabel = batch.files.length === 1 ? currentFile?.label ?? null : null;
      allAvailableFormats = orderTargets(batch.common_targets, batch.files);
      page = 0;
      updatePagedFormats();
      return true;
    } catch (error) {
      if (destroyed || revision !== fileRevision) return false;
      sourceFormat = null;
      sourceFormatLabel = null;
      activeIndex = -1;
      conversionError = String(error);
      status = 'error';
      playSound('convert_error');
      return false;
    }
  }

  async function startConversion(targetFmt: TargetFormatInfo, index: number) {
    if (!currentFilePaths.length || settingsVisible || status === 'converting' || !targetFmt.available) return;
    const paths = [...currentFilePaths];
    const conversionOptions = structuredClone(options);
    conversionError = '';
    clearDragDwell();
    clearTimeout(statusTimer);
    convertingIndex = index;
    activeIndex = index;
    status = 'converting';
    progress = 0;
    batchCompleted = 0;
    batchTotal = paths.length;
    stopping = false;
    cancelClose();
    wheelVisible = true;
    playSound('convert_start');
    try {
      await setConvertingState(true);
      const result = await runConversionBatch(paths, {
        stopped: () => stopping || destroyed,
        convert: (path, _index) => {
          activeJobId = crypto.randomUUID();
          return convertFile(activeJobId, path, targetFmt.format, conversionOptions);
        },
        completed: (count) => { batchCompleted = count; progress = count / batchTotal; },
      });
      if (destroyed) return;
      if (result.failures.length) {
        conversionError = result.failures.map(({ path, error }) => `${path.split(/[/\\]/).pop()}: ${error}`).join('\n');
        status = 'error';
        playSound('convert_error');
        // Keep errors visible until the user dismisses or retries.
      } else if (result.cancelled) {
        status = 'idle';
        convertingIndex = -1;
        activeIndex = -1;
        progress = 0;
        await hideWheel();
      } else {
        status = 'done';
        progress = 1;
        playSound('convert_done');
        statusTimer = setTimeout(() => {
          status = 'idle';
          convertingIndex = -1;
          void hideWheel();
        }, 1200);
      }
    } catch (error) {
      conversionError = String(error);
      status = 'error';
      playSound('convert_error');
    } finally {
      activeJobId = null;
      stopping = false;
      await setConvertingState(false);
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
    await scopedProgress((payload) => {
      if (payload.job_id !== activeJobId) return;
      if (stopping) {
        // Also covers a stop click before the conversion command registered
        // its token. Progress is emitted only after registration.
        void cancelJob(payload.job_id).catch(error => console.warn('Cancel failed:', error));
        return;
      }
      if (payload.status === 'converting') {
        const nextProgress = (batchCompleted + payload.progress) / batchTotal;
        if (Math.abs(nextProgress - progress) >= 0.002) progress = nextProgress;
      }
    });

    if (destroyed) return;
    // Tauri drag-and-drop & global shortcut events
    if (isTauri) {
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        const { listen: listenEvent } = await import('@tauri-apps/api/event');
        const listen = <T,>(name: string, callback: (event: { payload: T }) => void) =>
          listenEvent<T>(name, callback).then(keepListener);
        const webview = getCurrentWebview();
        await listen('open_advanced_settings', () => { void openSettings(); });

        await listen<{ generation: number; focus: boolean }>('wheel_shown', (event) => {
          nativeGeneration = event.payload.generation;
          cachedWheelRect = null;
          dragRevision++;
          dragCancelled = false;
          cancelClose();
        });
        await listen('wheel_close_requested', () => {
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
        await listen('shift_drag_start', () => {
          if (settingsVisible) return;
          cachedWheelRect = null;
          dragRevision++;
          fileRevision++;
          loadingFiles = null;
          dragCancelled = false;
          dragReleased = false;
          dragInsideWebview = false;
          dropAccepted = false;
          isDragging = true;
        });

        await listen('shift_drag_end', () => {
          if (settingsVisible) return;
          dragReleased = true;
          // Mouse-up is not an OLE Drop. Keep the target alive until Drop/Leave,
          // including when a drop is still waiting for format detection.
          if (!dragInsideWebview && !dropAccepted && status !== 'converting' && status !== 'done') {
            hideWheel();
          }
        });

        await listen('drag_cancelled', () => {
          if (settingsVisible) return;
          if (status !== 'converting') {
            hideWheel();
          }
        });

        // 2. Tauri webview drag-and-drop events (triggered when files hover over webview)
        await webview.onDragDropEvent(async (event) => {
          if (settingsVisible || dragCancelled || destroyed) return;
          if (status === 'converting' || status === 'done') {
            clearDragDwell();
            return;
          }

          const dpr = window.devicePixelRatio || 1;

          if (event.payload.type === 'enter') {
            const revision = ++dragRevision;
            const paths = event.payload.paths;
            dragInsideWebview = true;
            dragReleased = false;
            dropAccepted = false;
            isDragging = true;
            const pendingFiles = { paths: [...paths], ready: handleFileLoaded(paths) };
            loadingFiles = pendingFiles;
            await pendingFiles.ready;
            if (!isCurrentDrag(revision) || !isDragging || wheelClosing) return;
            await showWheel();
          } else if (event.payload.type === 'over') {
            if (wheelClosing) return;
            dragInsideWebview = true;
            if (!wheelVisible) {
              await showWheel();
            }
            const pos = event.payload.position;
            // Convert physical coordinates to CSS logical coordinates
            const cssX = pos.x / dpr;
            const cssY = pos.y / dpr;
            const hit = hitTestClient(cssX, cssY, getOpenWheelRect());
            const nextIndex = hit.type === 'petal' ? hit.index : -1;
            if (nextIndex !== activeIndex) {
              activeIndex = nextIndex;
              if (nextIndex >= 0) playSound('petal_hover');
            }

            if (hit.type === 'petal' && hit.index === 7 && hasMorePages) {
              triggerDragDwell();
            } else {
              clearDragDwell();
            }
          } else if (event.payload.type === 'drop') {
            if (dropAccepted) return;
            const revision = dragRevision;
            dropAccepted = true;
            const paths = event.payload.paths;
            const pos = event.payload.position;
            // Snapshot geometry before any await/transition can move the wheel.
            const hit = hitTestClient(pos.x / dpr, pos.y / dpr, getOpenWheelRect());
            cancelClose();
            dragInsideWebview = false;
            isDragging = false;
            clearDragDwell();

            const sameFiles = loadingFiles?.paths.length === paths.length &&
              loadingFiles.paths.every((path, i) => path === paths[i]);
            const ready = await (sameFiles ? loadingFiles!.ready : handleFileLoaded(paths));
            if (!isCurrentDrag(revision)) return;
            await showWheel();
            if (!isCurrentDrag(revision)) return;
            if (!ready) return; // Keep detection errors visible.

            // Dropping directly on the "..." petal flips page and stays open.
            if (hit.type === 'petal' && hit.index === 7 && hasMorePages) {
              handleNextPage();
              return;
            }

            if (hit.type === 'petal' && hit.index >= 0 && hit.index < formats.length) {
              const target = formats[hit.index];
              if (target && target.available) {
                conversionError = '';
                await startConversion(target, hit.index);
                return;
              }
            }
            await hideWheel();
          } else if (event.payload.type === 'leave') {
            dragInsideWebview = false;
            clearDragDwell();
            activeIndex = -1;
            // Leave while the button is held allows re-entry. Leave after the
            // release completes a drag that was dropped outside the webview.
            if (dragReleased && !dropAccepted) await hideWheel();
          }
        }).then(keepListener);
        await listen('update_available', () => {
          void checkForUpdates(false);
        });
      } catch (err) {
        console.warn('Failed to register Tauri event handlers:', err);
      }
    }

    // Auto-check for updates 10 seconds after startup in background
    if (destroyed) return;
    updateTimer = setTimeout(() => {
      void checkForUpdates(false);
    }, 10000);
  });

  onDestroy(() => {
    destroyed = true;
    for (const cleanup of ownedListeners) cleanup();
    ownedListeners.clear();
    fileRevision++;
    clearDragDwell();
    clearTimeout(statusTimer);
    clearTimeout(updateTimer);
    clearTimeout(dragCooldownTimer);
    if (activeJobId) void cancelJob(activeJobId);
    void setConvertingState(false);
    cancelClose();
    window.removeEventListener('keydown', handleKeyDown);
  });
  $: centerLabel = currentFilePaths.length > 1 ? `${$t("Tệp")} (${currentFilePaths.length})` : sourceFormatLabel;

  function handleSelectFormat(fmt: TargetFormatInfo, index: number) {
    void startConversion(fmt, index);
  }

  function handleHoverChange(idx: number) {
    if (idx !== activeIndex) {
      activeIndex = idx;
      if (idx >= 0) playSound('petal_hover');
    }
  }

  function handleCenterClick() {
    if (activeJobId) void stopConversion();
    else void openSettings();
  }
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
      {sourceFormat}
      {centerLabel}
      {hasMorePages}
      {page}
      {totalPages}
      onSelectFormat={handleSelectFormat}
      onHoverChange={handleHoverChange}
      onNextPage={handleNextPage}
      {stopping}
      onCenterClick={handleCenterClick}
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
  {#await import('./lib/OptionsPanel.svelte') then panel}
  <svelte:component this={panel.default}
    bind:this={optionsPanelRef}
    {options}
    onClose={closeSettings}
    onSave={(value) => {
      options = value;
    }}
  />
  {/await}
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
    pointer-events: none; /* Native window visibility owns desktop input capture. */
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
    background: var(--toast-bg, #000000);
    border: none;
    border-radius: 9999px;
    color: var(--toast-text, #ffffff);
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
    background: var(--hint-key-bg, #ffffff);
    border: none;
    border-radius: 6px;
    padding: 2px 8px;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.5px;
    color: var(--hint-key-text, #000000);
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
