<script lang="ts">
  export let value: number = 85;
  export let min: number = 0;
  export let max: number = 100;
  export let step: number = 1;
  export let unit: string = '%';
  export let ticks: number = 15;
  export let onChange: ((val: number) => void) | undefined = undefined;

  let containerEl: HTMLDivElement;
  let trackEl: HTMLDivElement;
  let isDragging = false;
  let justReleased = false;

  // Thumb half-width in px — used to clamp within the visible track
  const THUMB_W = 4;
  const THUMB_HALF = THUMB_W / 2;

  $: clamped = Math.min(max, Math.max(min, value));
  $: pct = Math.max(0, Math.min(100, ((clamped - min) / (max - min)) * 100));

  /**
   * Convert a clientX into a value, constrained strictly to the track element.
   * We use trackEl (not containerEl) so the value-readout column is excluded.
   */
  function valFromPointer(clientX: number): number {
    if (!trackEl) return value;
    const rect = trackEl.getBoundingClientRect();
    // clamp into [left+THUMB_HALF, right-THUMB_HALF] so edges are reachable
    const usable = rect.width - THUMB_HALF * 2;
    const raw = Math.max(0, Math.min(usable, clientX - rect.left - THUMB_HALF));
    const ratio = raw / usable;
    const rawVal = min + ratio * (max - min);
    return Math.min(max, Math.max(min, Math.round(rawVal / step) * step));
  }

  function handlePointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    isDragging = true;
    justReleased = false;
    containerEl.setPointerCapture(e.pointerId);
    const next = valFromPointer(e.clientX);
    if (next !== value) { value = next; onChange?.(value); }
  }

  function handlePointerMove(e: PointerEvent) {
    if (!isDragging) return;
    const next = valFromPointer(e.clientX);
    if (next !== value) { value = next; onChange?.(value); }
  }

  function handlePointerUp(e: PointerEvent) {
    if (!isDragging) return;
    isDragging = false;
    justReleased = true;
    try { containerEl.releasePointerCapture(e.pointerId); } catch {}
    // spring-back flicker: briefly flag "justReleased" for elastic pop animation
    setTimeout(() => { justReleased = false; }, 300);
  }

  function handleKeyDown(e: KeyboardEvent) {
    let next = value;
    if (e.key === 'ArrowRight' || e.key === 'ArrowUp') next += step;
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') next -= step;
    else if (e.key === 'Home') next = min;
    else if (e.key === 'End') next = max;
    else return;
    e.preventDefault();
    value = Math.min(max, Math.max(min, next));
    onChange?.(value);
  }
</script>

<!--
  Outer container captures all pointer events.
  slider-track is the actual drag area — value readout is a sibling column.
-->
<div
  class="slider-root"
  class:dragging={isDragging}
  class:released={justReleased}
  bind:this={containerEl}
  role="slider"
  tabindex="0"
  aria-valuenow={value}
  aria-valuemin={min}
  aria-valuemax={max}
  on:keydown={handleKeyDown}
  on:pointerdown={handlePointerDown}
  on:pointermove={handlePointerMove}
  on:pointerup={handlePointerUp}
  on:pointercancel={handlePointerUp}
>
  <!-- Track: only this rect is used for position math -->
  <div class="slider-track" bind:this={trackEl}>
    <!-- Fill region left of thumb -->
    <div class="track-fill" style="width: {pct}%;"></div>

    <!-- Ruler ticks — evenly spaced, independent of edges -->
    <div class="ticks-layer">
      {#each Array(ticks) as _, i}
        <span
          class="tick"
          class:tick-past={i / (ticks - 1) <= pct / 100}
          class:tick-major={i === 0 || i === ticks - 1 || i % Math.ceil(ticks / 4) === 0}
        ></span>
      {/each}
    </div>

    <!--
      Thumb: positioned with left clamped by CSS padding so it never
      overflows the visible track. No CSS transition during drag.
    -->
    <div
      class="thumb-wrapper"
      style="left: clamp({THUMB_HALF}px, calc({pct}% - 1px), calc(100% - {THUMB_HALF}px))"
    >
      <div class="thumb-bar"></div>
    </div>
  </div>

  <!-- Value readout — not part of drag math -->
  <div class="value-col" aria-hidden="true">
    <span class="val-number">{clamped}</span>
    {#if unit}
      <span class="val-unit">{unit}</span>
    {/if}
  </div>
</div>

<style>
  /* ─── Root Container ──────────────────────────────────────────────────────── */
  .slider-root {
    width: 100%;
    height: 36px;
    display: flex;
    align-items: stretch;
    border-radius: 11px;
    border: 1px solid rgba(255, 255, 255, 0.07);
    background: rgba(255, 255, 255, 0.035);
    overflow: hidden;
    user-select: none;
    cursor: pointer;
    outline: none;
    transition:
      border-color 0.2s cubic-bezier(0.16, 1, 0.3, 1),
      background   0.2s cubic-bezier(0.16, 1, 0.3, 1),
      box-shadow   0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .slider-root:hover {
    border-color: rgba(255, 255, 255, 0.14);
    background: rgba(255, 255, 255, 0.045);
  }

  .slider-root:focus-visible {
    border-color: rgba(255, 255, 255, 0.35);
    box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.12);
  }

  .slider-root.dragging {
    border-color: rgba(255, 255, 255, 0.28);
    background: rgba(255, 255, 255, 0.05);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.06) inset;
    cursor: grabbing;
  }

  /* ─── Track Area ──────────────────────────────────────────────────────────── */
  .slider-track {
    position: relative;
    flex: 1;
    /* No overflow:hidden → thumb won't get clipped at 0% or 100% */
  }

  /* ─── Fill Region ─────────────────────────────────────────────────────────── */
  .track-fill {
    position: absolute;
    inset: 0;
    right: auto; /* width is set inline */
    background: rgba(255, 255, 255, 0.09);
    pointer-events: none;
    /* Transition only when NOT dragging */
    transition: width 0.08s linear;
  }

  /* Suppress fill transition while dragging (applied via parent class) */
  .slider-root.dragging .track-fill {
    transition: none;
  }

  /* Spring pop on release */
  .slider-root.released .track-fill {
    transition: width 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  /* ─── Ruler Ticks (Hidden for sleek V-Notch pill design) ────────────────── */
  .ticks-layer {
    display: none;
  }

  .tick {
    width: 1px;
    height: 7px;
    background: rgba(255, 255, 255, 0.12);
    border-radius: 1px;
    flex-shrink: 0;
    transition: background 0.15s ease, height 0.15s ease;
  }

  .tick.tick-major {
    height: 12px;
    background: rgba(255, 255, 255, 0.18);
  }

  .tick.tick-past {
    background: rgba(255, 255, 255, 0.32);
  }

  .tick.tick-major.tick-past {
    background: rgba(255, 255, 255, 0.45);
    height: 14px;
  }

  /* ─── Thumb ───────────────────────────────────────────────────────────────── */
  .thumb-wrapper {
    position: absolute;
    top: 50%;
    /* left is set inline; transform centers the bar on that point */
    transform: translate(-50%, -50%);
    pointer-events: none;
    z-index: 3;
    display: flex;
    align-items: center;
    justify-content: center;
    /* Transition only when not dragging */
    transition: left 0.08s linear;
  }

  .slider-root.dragging .thumb-wrapper {
    transition: none;
  }

  .slider-root.released .thumb-wrapper {
    transition: left 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .thumb-bar {
    width: 3.5px;
    height: 18px;
    background: #ffffff;
    border-radius: 2.5px;
    box-shadow:
      0 0 7px rgba(255, 255, 255, 0.65),
      0 2px 5px rgba(0, 0, 0, 0.55);
    /* Smooth scale/glow transitions — these are fine even during drag */
    transition:
      height     0.18s cubic-bezier(0.34, 1.56, 0.64, 1),
      transform  0.18s cubic-bezier(0.34, 1.56, 0.64, 1),
      box-shadow 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .slider-root:hover .thumb-bar {
    height: 21px;
    box-shadow:
      0 0 10px rgba(255, 255, 255, 0.8),
      0 2px 6px rgba(0, 0, 0, 0.55);
  }

  .slider-root.dragging .thumb-bar {
    height: 24px;
    transform: scaleX(1.2);
    box-shadow:
      0 0 14px rgba(255, 255, 255, 0.95),
      0 0 28px rgba(255, 255, 255, 0.2),
      0 2px 8px rgba(0, 0, 0, 0.5);
  }

  /* Elastic spring-back: thumb briefly over-extends then settles */
  .slider-root.released .thumb-bar {
    animation: thumbSpring 0.28s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @keyframes thumbSpring {
    0%   { transform: scaleX(1.2) scaleY(1.0); }
    40%  { transform: scaleX(0.82) scaleY(1.08); }
    70%  { transform: scaleX(1.06) scaleY(0.97); }
    100% { transform: scaleX(1.0) scaleY(1.0); }
  }

  /* ─── Value Readout Column ────────────────────────────────────────────────── */
  .value-col {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 12px;
    border-left: 1px solid rgba(255, 255, 255, 0.07);
    flex-shrink: 0;
    min-width: 58px;
    justify-content: flex-end;
    pointer-events: none;
    font-variant-numeric: tabular-nums;
  }

  .val-number {
    font-size: 12px;
    font-weight: 700;
    color: #ffffff;
    transition: color 0.15s ease;
  }

  .slider-root.dragging .val-number {
    color: rgba(255, 255, 255, 0.85);
  }

  .val-unit {
    font-size: 9.5px;
    font-weight: 700;
    color: #71717a;
    letter-spacing: 0.02em;
  }
</style>
