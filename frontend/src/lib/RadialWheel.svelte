<script lang="ts">
  import { t } from './i18n';
  import { createPetalPath, getPetalCenter, hitTest } from './geometry';
  import type { TargetFormatInfo } from './types';

  export let formats: TargetFormatInfo[] = [];
  export let activeIndex: number = -1;
  export let convertingIndex: number = -1;
  export let progress: number = 0;
  export let stopping = false;
  export let status: 'idle' | 'converting' | 'done' | 'error' = 'idle';
  export let sourceFormat: string | null = null;
  export let hasMorePages: boolean = false;
  export let page: number = 0;
  export let totalPages: number = 1;
  export let onSelectFormat: (format: TargetFormatInfo) => void = () => {};
  export let onNextPage: () => void = () => {};
  export let onCenterClick: () => void = () => {};
  export let onHoverChange: (index: number) => void = () => {};

  const TOTAL_PETALS = 8;
  const CX = 260;
  const CY = 260;
  const R_IN = 70;
  const R_OUT = 244;

  $: activeFormat = activeIndex >= 0 && activeIndex < formats.length ? formats[activeIndex] : null;

  let dwellTimer: any = null;
  let dwellCooldown = false;

  function triggerDwell() {
    if (status === 'converting' || status === 'done') return;
    if (dwellCooldown || !hasMorePages) return;
    if (!dwellTimer) {
      dwellTimer = setTimeout(() => {
        onNextPage();
        dwellTimer = null;
        dwellCooldown = true;
        setTimeout(() => {
          dwellCooldown = false;
        }, 800);
      }, 280);
    }
  }

  function clearDwell() {
    if (dwellTimer) {
      clearTimeout(dwellTimer);
      dwellTimer = null;
    }
  }

  function handlePointerMove(e: PointerEvent) {
    if (status === 'converting' || status === 'done') {
      clearDwell();
      return;
    }

    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const hit = hitTest(x, y, CX, CY, R_IN, R_OUT, TOTAL_PETALS);

    if (hit.type === 'petal') {
      if (hit.index !== activeIndex) {
        onHoverChange(hit.index);
      }
      if (hit.index === 7 && hasMorePages) {
        triggerDwell();
      } else {
        clearDwell();
      }
    } else {
      clearDwell();
      if (activeIndex !== -1) {
        onHoverChange(-1);
      }
    }
  }

  function handlePointerLeave() {
    clearDwell();
    if (status !== 'converting' && status !== 'done') {
      onHoverChange(-1);
    }
  }

  function handleClick(index: number) {
    if (status === 'converting' || status === 'done') return;
    clearDwell();
    if (index === 7 && hasMorePages) {
      onNextPage();
      return;
    }
    const fmt = formats[index];
    if (fmt && fmt.available) {
      onSelectFormat(fmt);
    }
  }
</script>

<div
  class="wheel-container"
  role="region"
  aria-label={$t("Vertex – Chuyển đổi tệp")}
  on:pointermove={handlePointerMove}
  on:pointerleave={handlePointerLeave}
>
  <svg
    viewBox="0 0 520 520"
    width="520"
    height="520"
    class="wheel-svg"
  >
    <defs>
      <!-- Shadow for normal petals -->
      <filter id="petal-shadow" x="-30%" y="-30%" width="160%" height="160%">
        <feDropShadow dx="0" dy="4" stdDeviation="7" flood-color="#000000" flood-opacity="0.6" />
      </filter>

      <!-- Deep elevated shadow for active/hovered petal -->
      <filter id="active-petal-shadow" x="-40%" y="-40%" width="180%" height="180%">
        <feDropShadow dx="0" dy="8" stdDeviation="14" flood-color="#000000" flood-opacity="0.8" />
      </filter>

      <!-- Shadow for center pill -->
      <filter id="pill-shadow" x="-40%" y="-40%" width="180%" height="180%">
        <feDropShadow dx="0" dy="3" stdDeviation="6" flood-color="#000000" flood-opacity="0.5" />
      </filter>

      <!-- Elevated shadow for active center pill -->
      <filter id="active-pill-shadow" x="-50%" y="-50%" width="200%" height="200%">
        <feDropShadow dx="0" dy="6" stdDeviation="10" flood-color="#000000" flood-opacity="0.75" />
      </filter>

      <!-- Brilliant white glow filter for converting progress line -->
      <filter id="white-progress-glow" x="-60%" y="-60%" width="220%" height="220%">
        <feGaussianBlur in="SourceGraphic" stdDeviation="2" result="glow1" />
        <feGaussianBlur in="SourceGraphic" stdDeviation="5" result="glow2" />
        <feGaussianBlur in="SourceGraphic" stdDeviation="9" result="glow3" />
        <feMerge>
          <feMergeNode in="glow3" />
          <feMergeNode in="glow2" />
          <feMergeNode in="glow1" />
          <feMergeNode in="SourceGraphic" />
        </feMerge>
      </filter>
    </defs>

    <!-- The 8 Petals -->
    {#key page}
    {#each Array(TOTAL_PETALS) as _, i}
      {@const isMore = i === 7 && hasMorePages}
      {@const fmt = !isMore && i < formats.length ? formats[i] : null}
      {@const label = isMore ? '···' : fmt ? fmt.label : ''}
      {@const isAvailable = isMore || (fmt ? fmt.available : false)}
      {@const isActive = activeIndex === i}
      {@const isConverting = convertingIndex === i && status === 'converting'}
      {@const isDone = convertingIndex === i && status === 'done'}
      {@const isError = convertingIndex === i && status === 'error'}
      {@const pathD = createPetalPath(i, TOTAL_PETALS, CX, CY, R_IN, R_OUT, 14)}
      {@const centerPos = getPetalCenter(i, TOTAL_PETALS, CX, CY, 158)}

      {#if label}
        <g
          class="petal-group"
          class:active={isActive}
          class:converting={isConverting}
          class:done={isDone}
          class:error={isError}
          class:disabled={!isAvailable}
          style="--delay: {i * 35}ms;"
          on:click={() => handleClick(i)}
          on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && handleClick(i)}
          role="button"
          tabindex={0}
          aria-label={fmt?.format === sourceFormat ? `${label}: ${$t("Giữ định dạng, xử lý lại")}` : label}
        >
          <!-- Petal sector shape -->
          <path
            d={pathD}
            class="petal-shape"
          />

          <!-- Glowing white progress line running around the converting/done petal perimeter -->
          {#if isConverting || isDone}
            <path
              d={pathD}
              class="petal-progress-glow"
              class:done={isDone}
              pathLength="100"
              stroke-dasharray="100"
              stroke-dashoffset={isDone ? 0 : Math.max(0, Math.min(97, 100 - (progress * 100)))}
            />
          {/if}

          <!-- Petal Label or Status Icon -->
          <g transform="translate({centerPos.x}, {centerPos.y})">
            {#if isConverting}
              <!-- Format label with live progress percentage -->
              <text
                text-anchor="middle"
                y="-6"
                class="petal-text-converting"
              >
                {label}
              </text>
              <text
                text-anchor="middle"
                y="14"
                class="petal-text-percent"
              >
                {Math.round(progress * 100)}%
              </text>
            {:else if isDone}
              <!-- Checkmark icon -->
              <text
                text-anchor="middle"
                dominant-baseline="central"
                class="petal-text-done"
              >
                ✓
              </text>
            {:else if isError}
              <!-- Error X -->
              <text
                text-anchor="middle"
                dominant-baseline="central"
                class="petal-text-error"
              >
                ✕
              </text>
            {:else}
              <!-- Format Name -->
              <text
                text-anchor="middle"
                dominant-baseline="central"
                class="petal-text"
              >
                {label}
              </text>
            {/if}
          </g>
        </g>
      {/if}
    {/each}
    {/key}

    <!-- Center Hub Inner Glass Circle -->
    <circle
      cx={CX}
      cy={CY}
      r={R_IN - 4}
      class="center-hub"
      role="button"
      tabindex={0}
      aria-label={$t(status === "converting" ? (stopping ? "Đang dừng…" : "Dừng") : "Mở cài đặt nâng cao")}
      aria-disabled={stopping}
      on:click={onCenterClick}
      on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && onCenterClick()}
    />

    <!-- Center Pill / Capsule -->
    <g
      class="center-pill-group"
      class:has-active={activeFormat !== null}
      transform="translate({CX}, {CY})"
      role="button"
      tabindex={0}
      aria-label={$t(status === "converting" ? (stopping ? "Đang dừng…" : "Dừng") : "Mở cài đặt nâng cao")}
      aria-disabled={stopping}
      on:click={onCenterClick}
      on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && onCenterClick()}
    >
      <rect
        x="-58"
        y="-20"
        width="116"
        height="40"
        rx="20"
        class="center-pill-rect"
      />
      <text
        text-anchor="middle"
        dominant-baseline="central"
        class="center-pill-text"
      >
        {#if status === 'converting'}
          <tspan font-size={stopping ? 13 : 18}>■ {$t(stopping ? "Đang dừng…" : "Dừng")}</tspan>
        {:else if status === 'done'}
          ✓ {$t("Xong")}
        {:else if activeFormat}
          <tspan x="0" y="-5" font-size="18" font-weight="800">{activeFormat.label}</tspan>
          <tspan x="0" y="12" font-size="11" font-weight="700" letter-spacing="0.3px" opacity="0.65">
            {$t(activeFormat.format === sourceFormat ? 'Nén lại' : activeFormat.category === 'image' ? 'Ảnh' : activeFormat.category === 'audio' ? 'Âm thanh' : activeFormat.category === 'video' ? 'Video' : activeFormat.category === 'document' ? 'Tài liệu' : 'Định dạng')}
          </tspan>
        {:else if activeIndex === 7 && hasMorePages}
          <tspan x="0" y="-5" font-size="16" font-weight="800">{$t("Trang")} {(page + 1) % totalPages + 1}/{totalPages}</tspan>
          <tspan x="0" y="12" font-size="11" font-weight="700" letter-spacing="0.3px" opacity="0.65">{$t("Trang tiếp")}</tspan>
        {:else if sourceFormat}
          {sourceFormat}
        {:else}
          VERTEX
        {/if}
      </text>

      <!-- Page dots indicator when multi-page -->
      {#if hasMorePages && totalPages > 1}
        <g class="page-dots" transform="translate(0, 13)">
          {#each Array(totalPages) as _, pIdx}
            <circle
              cx={(pIdx - (totalPages - 1) / 2) * 7}
              cy="0"
              r={pIdx === page ? 2 : 1.2}
              fill={activeFormat ? "#000000" : "#ffffff"}
              opacity={pIdx === page ? 0.9 : 0.3}
            />
          {/each}
        </g>
      {/if}
    </g>
  </svg>

  <!-- Missing Engine Tooltip -->
  {#if activeFormat && !activeFormat.available && activeFormat.reason}
    <div class="tooltip-container">
      <div class="tooltip-glass">
        <span class="tooltip-icon">⚠️</span>
        <span>{$t(activeFormat.reason.toLowerCase().includes("ffmpeg") ? "Cần cài FFmpeg để dùng định dạng này." : "Định dạng này chưa dùng được.")}</span>
      </div>
    </div>
  {/if}
</div>

<style>
  .wheel-container {
    position: relative;
    width: 520px;
    height: 520px;
    display: flex;
    align-items: center;
    justify-content: center;
    filter: drop-shadow(0 20px 48px rgba(0, 0, 0, 0.35));
  }

  .wheel-svg {
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  /* Petal Group */
  .petal-group {
    cursor: pointer;
    transform-origin: 260px 260px;
    animation: petalPopIn 0.4s cubic-bezier(0.16, 1, 0.3, 1) both;
    animation-delay: var(--delay, 0ms);
    transition: transform 0.18s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .petal-shape {
    fill: var(--petal-fill, #000000);
    stroke: none;
    filter: url(#petal-shadow);
    transition: fill 0.15s cubic-bezier(0.16, 1, 0.3, 1), filter 0.15s ease;
  }

  .petal-text {
    font-family: var(--font-family);
    font-size: 22px;
    font-weight: 700;
    letter-spacing: 0.8px;
    fill: var(--petal-text, #ffffff);
    transition: fill 0.15s ease;
    user-select: none;
    pointer-events: none;
  }

  /* Active / Hovered State: Inverted Solid White with bold black text & elevated shadow */
  .petal-group.active {
    transform: scale(1.035);
  }

  .petal-group.active .petal-shape {
    fill: var(--petal-hover-fill, #ffffff);
    stroke: none;
    filter: url(#active-petal-shadow);
  }

  .petal-group.active .petal-text {
    fill: var(--petal-hover-text, #000000);
    font-size: 23px;
    font-weight: 800;
  }

  /* Glowing white progress line running around petal perimeter */
  .petal-progress-glow {
    fill: none;
    stroke: #ffffff;
    stroke-width: 3.5;
    stroke-linecap: round;
    stroke-linejoin: round;
    filter: url(#white-progress-glow);
    transition: stroke-dashoffset 0.22s cubic-bezier(0.16, 1, 0.3, 1);
    pointer-events: none;
  }

  .petal-group.converting .petal-progress-glow {
    animation: progressGlowBreathe 1.5s infinite ease-in-out;
  }

  .petal-progress-glow.done {
    stroke-width: 4;
    transition: stroke-dashoffset 0.15s ease-out;
    animation: doneFlash 0.5s ease-out;
  }

  @keyframes progressGlowBreathe {
    0%, 100% {
      opacity: 0.9;
    }
    50% {
      opacity: 1;
    }
  }

  @keyframes doneFlash {
    0% {
      stroke-width: 3.5;
      opacity: 0.9;
    }
    50% {
      stroke-width: 5.5;
      opacity: 1;
    }
    100% {
      stroke-width: 4;
      opacity: 1;
    }
  }

  /* Converting State */
  .petal-group.converting .petal-shape {
    fill: #000000;
    stroke: none;
    filter: url(#active-petal-shadow);
  }

  .petal-text-converting {
    font-family: var(--font-family);
    font-size: 22px;
    font-weight: 700;
    letter-spacing: 0.5px;
    fill: #ffffff;
  }

  .petal-text-percent {
    font-family: var(--font-family);
    font-size: 16px;
    font-weight: 700;
    letter-spacing: 0.5px;
    fill: #a1a1aa;
  }

  /* Done State: Inverted solid black with white checkmark & glowing outline */
  .petal-group.done .petal-shape {
    fill: #000000;
    stroke: none;
    filter: url(#active-petal-shadow);
  }

  .petal-text-done {
    font-family: var(--font-family);
    font-size: 24px;
    font-weight: 800;
    fill: #ffffff;
    filter: drop-shadow(0 0 8px rgba(255, 255, 255, 0.9));
  }

  /* Error State: Solid black with crisp white X */
  .petal-group.error {
    animation: petalShake 0.4s ease-in-out;
  }

  .petal-group.error .petal-shape {
    fill: var(--petal-error-fill, #000000);
    stroke: none;
    filter: url(#petal-shadow);
  }

  .petal-text-error {
    font-family: var(--font-family);
    font-size: 18px;
    font-weight: 800;
    fill: var(--petal-error-text, #ffffff);
  }

  /* Disabled State */
  .petal-group.disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .petal-group.disabled .petal-shape {
    fill: var(--petal-disabled-fill, #0a0a0a);
    stroke: none;
    filter: none;
  }

  .petal-group.disabled .petal-text {
    fill: var(--petal-disabled-text, #52525b);
  }

  /* Center Hub */
  .center-hub {
    fill: var(--center-hub-fill, #000000);
    stroke: none;
    cursor: pointer;
    transition: transform 0.2s ease, opacity 0.2s ease;
  }

  .center-hub:hover {
    opacity: 0.9;
  }

  /* Center Pill */
  .center-pill-group {
    cursor: pointer;
    transition: transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .center-pill-group:hover {
    transform: translate(260px, 260px) scale(1.05);
  }

  .center-pill-rect {
    fill: var(--center-pill-fill, #000000);
    stroke: none;
    filter: url(#pill-shadow);
    transition: fill 0.15s ease, filter 0.15s ease;
  }

  .center-pill-group.has-active .center-pill-rect {
    fill: var(--center-pill-active-fill, #ffffff);
    stroke: none;
    filter: url(#active-pill-shadow);
  }

  .center-pill-text {
    font-family: var(--font-family);
    font-size: 18px;
    font-weight: 700;
    letter-spacing: 0.8px;
    fill: var(--center-pill-text, #ffffff);
    user-select: none;
    pointer-events: none;
    transition: fill 0.15s ease;
  }

  .center-pill-group.has-active .center-pill-text {
    fill: var(--center-pill-active-text, #000000);
    font-weight: 800;
  }

  /* Tooltip for disabled engine */
  .tooltip-container {
    position: absolute;
    top: -45px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 120;
    pointer-events: none;
    animation: toastSlideUp 0.2s ease-out;
  }

  .tooltip-glass {
    display: flex;
    align-items: center;
    gap: 8px;
    background: #000000;
    border: none;
    border-radius: 9999px;
    padding: 6px 14px;
    color: #ffffff;
    font-family: var(--font-family);
    font-size: 16px;
    font-weight: 600;
    white-space: nowrap;
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.75);
  }

  .tooltip-icon {
    font-size: 14px;
  }
</style>
