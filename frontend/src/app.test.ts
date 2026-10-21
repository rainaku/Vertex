import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';

const mocks = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  disposers: [] as ReturnType<typeof vi.fn>[],
  hide: vi.fn(async () => {}),
  prepare: vi.fn(async () => {}),
  passthrough: vi.fn(async (_enabled: boolean) => {}),
  restore: vi.fn(async () => {}),
  forceHide: vi.fn(async () => {}),
  dragDrop: null as null | ((event: { payload: { type: 'enter' | 'over' | 'drop' | 'leave'; paths?: string[]; position?: { x: number; y: number } } }) => Promise<void>),
  detect: vi.fn(async (_path: string) => ({ format: 'JPG', label: 'JPG', category: 'image', extension: 'jpg', filename: 'photo.jpg' })),
  targets: vi.fn(async () => [
    { format: 'JPG', label: 'JPG', category: 'image', extension: 'jpg', available: true },
    { format: 'PNG', label: 'PNG', category: 'image', extension: 'png', available: true },
  ]),
  convert: vi.fn(async (_job: string, _path: string, target: string) => ({ success: true, cancelled: false, output_path: 'photo.png', target_format: target })),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name, callback) => {
    mocks.listeners.set(name, callback);
    const dispose = vi.fn();
    mocks.disposers.push(dispose);
    return dispose;
  }),
}));
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({
  onDragDropEvent: async (callback: typeof mocks.dragDrop) => { mocks.dragDrop = callback; return vi.fn(); },
}) }));
vi.mock('@tauri-apps/plugin-autostart', () => ({ isEnabled: async () => false, enable: vi.fn(), disable: vi.fn() }));
vi.mock('./lib/tauri-bridge', () => ({
  isTauri: true, detectFile: mocks.detect, getAvailableTargets: mocks.targets, getBatchInfo: vi.fn(),
  convertFile: mocks.convert, cancelJob: vi.fn(), listenProgress: async () => vi.fn(),
  setWindowPassthrough: mocks.passthrough, hideWheelWindow: mocks.hide, forceHideWheelWindow: mocks.forceHide,
  setConvertingState: vi.fn(), prepareSettingsWindow: mocks.prepare, restoreWheelWindow: mocks.restore,
}));
vi.mock('./lib/updater', async () => {
  const { writable } = await import('svelte/store');
  return { checkForUpdates: vi.fn(), installUpdate: vi.fn(), openReleasePage: vi.fn(), updateStore: writable({ currentVersion: '0.1.0', status: 'idle' }) };
});
import App from './App.svelte';

afterEach(() => {
  cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); vi.clearAllMocks(); mocks.listeners.clear(); mocks.disposers.length = 0;
  mocks.dragDrop = null;
  delete (HTMLElement.prototype as unknown as { getAnimations?: unknown }).getAnimations;
});

it('opening Settings invalidates a pending wheel exit and releases all native listeners', async () => {
  Object.defineProperty(HTMLElement.prototype, 'getAnimations', { configurable: true, value: () => [{ finished: new Promise(() => {}) }] });
  const view = render(App);
  await waitFor(() => expect(mocks.listeners.has('update_available')).toBe(true));
  mocks.listeners.get('wheel_shown')!({ payload: { generation: 1, focus: false } });
  mocks.listeners.get('shift_drag_start')!({ payload: undefined });
  await mocks.dragDrop!({ payload: { type: 'enter', paths: ['C:\\input\\photo.jpg'], position: { x: 316, y: 208 } } });
  await tick();
  await fireEvent.keyDown(window, { key: 'Escape' });
  await tick();
  mocks.listeners.get('open_advanced_settings')!({ payload: undefined });
  await waitFor(() => expect(view.getByRole('dialog')).toBeTruthy());
  await new Promise(resolve => setTimeout(resolve, 350));
  expect(mocks.prepare).toHaveBeenCalledTimes(1);
  expect(mocks.hide).not.toHaveBeenCalled();
  view.unmount();
  for (const dispose of mocks.disposers) expect(dispose).toHaveBeenCalledTimes(1);
  delete (HTMLElement.prototype as unknown as { getAnimations?: unknown }).getAnimations;
});

it.each([1, 1.5, 2])('a native mouse release before the OLE drop still converts the selected petal (DPI scale %s)', async dpr => {
  vi.stubGlobal('devicePixelRatio', dpr);
  Object.defineProperty(HTMLElement.prototype, 'getAnimations', { configurable: true, value: () => [] });
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(new DOMRect(97, 97, 286, 286));
  const view = render(App);
  await waitFor(() => expect(mocks.listeners.has('update_available')).toBe(true));
  mocks.listeners.get('wheel_shown')!({ payload: { generation: 1, focus: false } });
  mocks.listeners.get('shift_drag_start')!({ payload: undefined });
  const paths = ['C:\\input\\photo.jpg'];
  const position = { x: 316 * dpr, y: 208 * dpr }; // PNG, petal index 1 in the scaled wheel.
  await mocks.dragDrop!({ payload: { type: 'enter', paths, position } });
  await tick();
  await mocks.dragDrop!({ payload: { type: 'over', position } });
  mocks.listeners.get('shift_drag_end')!({ payload: undefined });
  await mocks.dragDrop!({ payload: { type: 'drop', paths, position } });
  await waitFor(() => expect(mocks.convert).toHaveBeenCalledWith(expect.any(String), paths[0], 'PNG', expect.any(Object)));
  expect(mocks.hide).not.toHaveBeenCalled();
  expect(mocks.convert).toHaveBeenCalledTimes(1);
  expect(mocks.detect).toHaveBeenCalledTimes(1);
  view.unmount();
});

async function mountDragWheel() {
  Object.defineProperty(HTMLElement.prototype, 'getAnimations', { configurable: true, value: () => [] });
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(new DOMRect(97, 97, 286, 286));
  const view = render(App);
  await waitFor(() => expect(mocks.listeners.has('update_available')).toBe(true));
  mocks.listeners.get('wheel_shown')!({ payload: { generation: 1, focus: false } });
  mocks.listeners.get('shift_drag_start')!({ payload: undefined });
  return view;
}

const dragPaths = ['C:\\input\\photo.jpg'];
const pngPosition = { x: 316, y: 208 };

it('OLE drop remains accepted when detection is pending and duplicate Drop/release events arrive', async () => {
  let finishDetection!: (value: Awaited<ReturnType<typeof mocks.detect>>) => void;
  mocks.detect.mockImplementationOnce(() => new Promise(resolve => { finishDetection = resolve; }));
  await mountDragWheel();
  const enter = mocks.dragDrop!({ payload: { type: 'enter', paths: dragPaths, position: pngPosition } });
  const firstDrop = mocks.dragDrop!({ payload: { type: 'drop', paths: dragPaths, position: pngPosition } });
  mocks.listeners.get('shift_drag_end')!({ payload: undefined });
  await mocks.dragDrop!({ payload: { type: 'drop', paths: dragPaths, position: pngPosition } });
  expect(mocks.convert).not.toHaveBeenCalled();
  finishDetection({ format: 'JPG', label: 'JPG', category: 'image', extension: 'jpg', filename: 'photo.jpg' });
  await Promise.all([enter, firstDrop]);
  expect(mocks.convert).toHaveBeenCalledTimes(1);
  expect(mocks.convert.mock.calls[0][2]).toBe('PNG');
  mocks.listeners.get('shift_drag_end')!({ payload: undefined });
  await tick();
  expect(mocks.hide).not.toHaveBeenCalled();
});

it.each(['leave-first', 'release-first'])('dropping outside the webview closes once (%s)', async order => {
  await mountDragWheel();
  await mocks.dragDrop!({ payload: { type: 'enter', paths: dragPaths, position: pngPosition } });
  const release = () => mocks.listeners.get('shift_drag_end')!({ payload: undefined });
  const leave = () => mocks.dragDrop!({ payload: { type: 'leave' } });
  if (order === 'leave-first') { await leave(); expect(mocks.hide).not.toHaveBeenCalled(); release(); }
  else { release(); expect(mocks.hide).not.toHaveBeenCalled(); await leave(); }
  await waitFor(() => expect(mocks.hide).toHaveBeenCalledTimes(1));
  expect(mocks.convert).not.toHaveBeenCalled();
});

it('Escape cancels a drop waiting for detection and ignores its later completion', async () => {
  let finishDetection!: (value: Awaited<ReturnType<typeof mocks.detect>>) => void;
  mocks.detect.mockImplementationOnce(() => new Promise(resolve => { finishDetection = resolve; }));
  await mountDragWheel();
  const enter = mocks.dragDrop!({ payload: { type: 'enter', paths: dragPaths, position: pngPosition } });
  const drop = mocks.dragDrop!({ payload: { type: 'drop', paths: dragPaths, position: pngPosition } });
  mocks.listeners.get('drag_cancelled')!({ payload: undefined });
  finishDetection({ format: 'JPG', label: 'JPG', category: 'image', extension: 'jpg', filename: 'photo.jpg' });
  await Promise.all([enter, drop]);
  await mocks.dragDrop!({ payload: { type: 'drop', paths: dragPaths, position: pngPosition } });
  expect(mocks.convert).not.toHaveBeenCalled();
  await waitFor(() => expect(mocks.hide).toHaveBeenCalledTimes(1));
});

it('a failed detection remains visible instead of silently discarding the drop', async () => {
  mocks.detect.mockRejectedValueOnce(new Error('Cannot read the dropped file'));
  const view = await mountDragWheel();
  await mocks.dragDrop!({ payload: { type: 'enter', paths: dragPaths, position: pngPosition } });
  mocks.listeners.get('shift_drag_end')!({ payload: undefined });
  await mocks.dragDrop!({ payload: { type: 'drop', paths: dragPaths, position: pngPosition } });
  expect(view.getByText('Error: Cannot read the dropped file')).toBeTruthy();
  expect(mocks.convert).not.toHaveBeenCalled();
  expect(mocks.hide).not.toHaveBeenCalled();
});

it.each([false, true])('cancelling while the native window opens invalidates the drop (new drag: %s)', async newDrag => {
  let finishOpening!: () => void;
  mocks.passthrough.mockImplementationOnce(() => new Promise(resolve => { finishOpening = resolve; }));
  await mountDragWheel();
  const staleDrop = mocks.dragDrop!({ payload: { type: 'drop', paths: dragPaths, position: pngPosition } });
  await waitFor(() => expect(mocks.passthrough).toHaveBeenCalledWith(false));
  mocks.listeners.get('drag_cancelled')!({ payload: undefined });
  if (newDrag) {
    mocks.listeners.get('wheel_shown')!({ payload: { generation: 2, focus: false } });
    mocks.listeners.get('shift_drag_start')!({ payload: undefined });
    await mocks.dragDrop!({ payload: { type: 'enter', paths: ['C:\\input\\next.jpg'], position: pngPosition } });
  }
  finishOpening();
  await staleDrop;
  expect(mocks.convert).not.toHaveBeenCalled();
});

it('Apply keeps Settings open so Reset and Save remain usable', async () => {
  const view = render(App);
  await waitFor(() => expect(mocks.listeners.has('open_advanced_settings')).toBe(true));
  mocks.listeners.get('open_advanced_settings')!({ payload: undefined });
  await waitFor(() => expect(view.getByRole('dialog')).toBeTruthy());
  await fireEvent.click(view.getByRole('button', { name: 'Apply' }));
  await waitFor(() => expect(view.getByRole('button', { name: /Applied/ })).toBeTruthy());
  expect(view.getByRole('dialog')).toBeTruthy();
  await fireEvent.click(view.getByRole('button', { name: 'Reset' }));
  await fireEvent.click(view.getByRole('button', { name: 'Save' }));
  await waitFor(() => expect(view.queryByRole('dialog')).toBeNull());
  expect(mocks.restore).toHaveBeenCalledTimes(1);
  expect(mocks.forceHide).not.toHaveBeenCalled();
});

it('repeated Settings requests prepare the native window once while it stays open', async () => {
  const view = render(App);
  await waitFor(() => expect(mocks.listeners.has('open_advanced_settings')).toBe(true));
  mocks.listeners.get('open_advanced_settings')!({ payload: undefined });
  await waitFor(() => expect(view.getByRole('dialog')).toBeTruthy());
  mocks.listeners.get('open_advanced_settings')!({ payload: undefined });
  mocks.listeners.get('open_advanced_settings')!({ payload: undefined });
  await tick();
  expect(mocks.prepare).toHaveBeenCalledTimes(1);
  expect(mocks.hide).not.toHaveBeenCalled();
  expect(mocks.forceHide).not.toHaveBeenCalled();
});
