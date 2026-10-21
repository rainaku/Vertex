<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import GearSixIcon from 'phosphor-svelte/lib/GearSixIcon';
  import PowerIcon from 'phosphor-svelte/lib/PowerIcon';

  type TrayState = { language: 'vi' | 'en'; version: string; converting: boolean };
  let state: TrayState = { language: 'vi', version: '', converting: false };
  let busy = false;
  let error = false;
  let menu: HTMLDivElement;
  const native = '__TAURI_INTERNALS__' in window;
  $: vi = state.language === 'vi';

  async function opened(value: TrayState) {
    state = value;
    busy = false;
    error = false;
    document.documentElement.lang = state.language;
    await tick();
    // Opening with the mouse must not activate the first item's focus effects.
    menu?.focus();
  }

  async function close() {
    if (native) await getCurrentWindow().hide();
  }

  async function act(action: 'settings' | 'quit') {
    if (busy) return;
    busy = true;
    error = false;
    try {
      if (native) await invoke('tray_action', { action });
    } catch (reason) {
      if (String(reason).includes('conversion_in_progress')) state = { ...state, converting: true };
      else error = true;
    } finally {
      busy = false;
    }
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); void close(); return; }
    const buttons = Array.from(menu.querySelectorAll<HTMLButtonElement>('button:not(:disabled)'));
    if (!buttons.length) return;
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    let next: number;
    if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = buttons.length - 1;
    else if (event.key === 'ArrowDown' || (event.key === 'Tab' && !event.shiftKey)) next = (index + 1) % buttons.length;
    else if (event.key === 'ArrowUp' || event.key === 'Tab') next = index < 0 ? buttons.length - 1 : (index - 1 + buttons.length) % buttons.length;
    else return;
    event.preventDefault();
    buttons[next].focus();
  }

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    if (native) {
      void (async () => {
        const cleanup = await listen<TrayState>('tray_opened', event => { void opened(event.payload); });
        if (disposed) { cleanup(); return; }
        unlisten = cleanup;
        const value = await invoke<TrayState>('get_tray_state');
        if (!disposed) await opened(value);
      })().catch(() => { if (!disposed) error = true; });
    }
    return () => { disposed = true; unlisten?.(); };
  });
</script>

<svelte:window on:keydown={keydown} />

<div class="tray-panel">
  <header>
    {#if error}
      <span class="error" role="alert">{vi ? 'Không thể thực hiện. Hãy thử lại.' : 'Unable to complete. Try again.'}</span>
    {:else}
      <span>Vertex</span>
      {#if state.version}<span class="version">v{state.version}</span>{/if}
    {/if}
  </header>

  <div class="actions" role="menu" aria-label="Vertex" tabindex="-1" bind:this={menu}>
    <button role="menuitem" disabled={busy || state.converting} title={state.converting ? (vi ? 'Đang chuyển đổi…' : 'Converting…') : undefined} on:click={() => act('settings')}>
      <span class="icon gear" aria-hidden="true"><GearSixIcon size={17} weight="fill" /></span>
      <span class="label">{vi ? 'Cài đặt' : 'Settings'}</span>
    </button>
    <div class="separator" role="separator"></div>
    <button class="quit" role="menuitem" disabled={busy} on:click={() => act('quit')}>
      <span class="icon power" aria-hidden="true"><PowerIcon size={17} weight="fill" /></span>
      <span class="label">{vi ? 'Thoát Vertex' : 'Quit Vertex'}</span>
    </button>
  </div>
</div>

<style>
  .tray-panel { width: min(248px, 100%); height: 122px; max-height: 100%; padding: 6px; overflow: hidden; background: #000000; color: #f4f4f5; border: 0; border-radius: 12px; font-family: var(--font-family); font-size: 13px; font-weight: 700; }
  header { display: flex; align-items: center; height: 27px; gap: 8px; padding: 0 9px; color: #909298; font-size: 11px; }
  .version { margin-left: auto; font-variant-numeric: tabular-nums; }
  .error { color: #fca5a5; font-size: 10px; }
  button { display: flex; width: 100%; height: 36px; align-items: center; gap: 10px; border: 0; border-radius: 7px; padding: 0 9px; background: transparent; color: #a8abb2; font: inherit; text-align: left; cursor: pointer; }
  .label { flex: 1; transition: color 120ms, text-shadow 120ms; }
  .icon { display: flex; flex: 0 0 17px; align-items: center; justify-content: center; transition: transform 220ms ease, color 120ms; }
  button:hover:not(:disabled) .label, button:focus-visible .label { color: #ffffff; text-shadow: 0 0 8px #ffffff60; }
  button:hover:not(:disabled) .gear, button:focus-visible .gear { color: #ffffff; animation: gear-turn 650ms cubic-bezier(.2, .7, .3, 1); }
  .quit { color: #f87171; }
  .quit:hover:not(:disabled) .label, .quit:focus-visible .label { color: #fca5a5; text-shadow: 0 0 8px #f8717160; }
  .quit:hover:not(:disabled) .power, .quit:focus-visible .power { color: #fca5a5; animation: power-pulse 320ms ease-out; }
  .actions:focus, button:focus-visible { outline: none; }
  button:disabled { opacity: .4; cursor: default; }
  .separator { height: 1px; margin: 4px 10px; background: #ffffff0a; }
  @keyframes gear-turn { from { transform: rotate(0deg); } to { transform: rotate(180deg); } }
  @keyframes power-pulse { 0%, 100% { transform: scale(1); } 50% { transform: scale(1.16); } }
  @media (prefers-reduced-motion: reduce) {
    .label, .icon { transition: none; }
    button:hover:not(:disabled) .icon, button:focus-visible .icon { animation: none; transform: none; }
  }
</style>
