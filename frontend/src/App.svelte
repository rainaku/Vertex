<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import RadialWheel from './lib/RadialWheel.svelte';
  import {
    detectFile,
    getAvailableTargets,
    getBatchInfo,
    convertFile,
    listenProgress,
    sendDesktopNotification,
    setWindowPassthrough,
    hideWheelWindow,
    setConvertingState,
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
  let status: 'idle' | 'converting' | 'done' | 'error' = 'idle';

  let currentFile: FormatInfo | null = null;
  let currentFilePaths: string[] = [];
  let sourceFormatLabel: string | null = null;

  // Whether wheel is currently visible
  let wheelVisible: boolean = false;

  // Whether a file is being dragged over our window
  let isDragging: boolean = false;

  // Pagination
  let page: number = 0;
  const PETALS_PER_PAGE = 7;

  // Options
  let options: Options = {
    quality: 85,
    dpi: 200,
    strip_metadata: true,
    collision_policy: 'rename_with_suffix',
  };

  let unlistenProgress: (() => void) | null = null;
  let unlistenDragDrop: (() => void) | null = null;
  let unlistenDragStart: (() => void) | null = null;
  let unlistenDragEnd: (() => void) | null = null;
  let unlistenDragCancel: (() => void) | null = null;

  // ─── Helpers ─────────────────────────────────────────────────────────────────

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
    if (wheelVisible) return;
    wheelVisible = true;
    // Disable passthrough so user can interact with the wheel
    await setWindowPassthrough(false);
  }

  async function hideWheel() {
    wheelVisible = false;
    activeIndex = -1;
    isDragging = false;
    // Re-enable passthrough and hide window
    await setWindowPassthrough(true);
    await hideWheelWindow();
  }

  // ─── File handling ───────────────────────────────────────────────────────────

  async function handleFileLoaded(paths: string[]) {
    if (paths.length === 0) return;
    currentFilePaths = paths;

    if (paths.length === 1) {
      try {
        const info = await detectFile(paths[0]);
        currentFile = info;
        sourceFormatLabel = info.label;
        const targets = await getAvailableTargets(info.format);
        allAvailableFormats = targets.length > 0 ? targets : [];
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
        allAvailableFormats = batch.common_targets;
        page = 0;
        updatePagedFormats();
      } catch (e) {
        console.error('Batch detection failed:', e);
      }
    }
  }

  // ─── Conversion ──────────────────────────────────────────────────────────────

  async function startConversion(targetFmt: TargetFormatInfo, index: number) {
    if (!currentFilePaths.length) return;

    clearDragDwell();
    convertingIndex = index;
    activeIndex = index;
    status = 'converting';
    progress = 0.05;
    await setConvertingState(true);

    const jobId = `job_${Date.now()}`;
    const firstPath = currentFilePaths[0];

    try {
      const res = await convertFile(jobId, firstPath, targetFmt.format, options);
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
      } else {
        status = 'error';
        setTimeout(async () => {
          status = 'idle';
          convertingIndex = -1;
          await setConvertingState(false);
        }, 2000);
      }
    } catch (e: any) {
      status = 'error';
      setTimeout(async () => {
        status = 'idle';
        convertingIndex = -1;
        await setConvertingState(false);
      }, 2000);
    }
  }

  // ─── Keyboard ────────────────────────────────────────────────────────────────

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      hideWheel();
    }
  }

  // ─── Lifecycle ───────────────────────────────────────────────────────────────

  onMount(async () => {
    window.addEventListener('keydown', handleKeyDown);

    // Subscribe to conversion progress
    unlistenProgress = await listenProgress((payload) => {
      progress = payload.progress;
      if (payload.status === 'done') {
        status = 'done';
      } else if (payload.status === 'error') {
        status = 'error';
      }
    });

    // Tauri drag-and-drop & global shortcut events
    if (isTauri) {
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        const { listen } = await import('@tauri-apps/api/event');
        const webview = getCurrentWebview();

        // 1. Listen for global Shift+Drag events from Rust background thread
        unlistenDragStart = await listen('shift_drag_start', () => {
          isDragging = true;
        });

        unlistenDragEnd = await listen('shift_drag_end', () => {
          if (status !== 'converting') {
            hideWheel();
          }
        });

        unlistenDragCancel = await listen('drag_cancelled', () => {
          if (status !== 'converting') {
            hideWheel();
          }
        });

        // 2. Tauri webview drag-and-drop events (triggered when files hover over webview)
        unlistenDragDrop = await webview.onDragDropEvent(async (event) => {
          if (status === 'converting' || status === 'done') {
            clearDragDwell();
            return;
          }

          const dpr = window.devicePixelRatio || 1;

          if (event.payload.type === 'enter') {
            const paths = event.payload.paths;
            isDragging = true;
            await handleFileLoaded(paths);
            await showWheel();
          } else if (event.payload.type === 'over') {
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
      } catch (err) {
        console.warn('Failed to register Tauri event handlers:', err);
      }
    }
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeyDown);
    if (unlistenProgress) unlistenProgress();
    if (unlistenDragDrop) unlistenDragDrop();
    if (unlistenDragStart) unlistenDragStart();
    if (unlistenDragEnd) unlistenDragEnd();
    if (unlistenDragCancel) unlistenDragCancel();
  });
</script>

<!-- 
  Ghost overlay: the entire viewport is transparent.
  The wheel fades in/out via CSS when wheelVisible changes.
-->
<main class="ghost-overlay">
  <!-- Radial Wheel — shown only when wheelVisible -->
  <div class="wheel-wrapper" class:visible={wheelVisible}>
    <RadialWheel
      {formats}
      {activeIndex}
      {convertingIndex}
      {progress}
      {status}
      sourceFormat={sourceFormatLabel}
      {hasMorePages}
      {page}
      {totalPages}
      onSelectFormat={(fmt) => startConversion(fmt, activeIndex)}
      onHoverChange={(idx) => activeIndex = idx}
      onNextPage={handleNextPage}
      onCenterClick={() => {}}
    />
  </div>

  <!-- Hint pill — shown only when dragging without Shift -->
  {#if isDragging && !wheelVisible}
    <div class="ctrl-hint">
      <span class="ctrl-key">Shift</span>
      <span>+ thả để chuyển đổi</span>
    </div>
  {/if}

</main>

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
