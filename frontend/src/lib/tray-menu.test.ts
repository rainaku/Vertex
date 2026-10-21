import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, waitFor } from '@testing-library/svelte';
import TrayMenu from './TrayMenu.svelte';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(), hide: vi.fn(), dispose: vi.fn(),
  opened: null as null | ((event: { payload: { language: string; version: string; converting: boolean } }) => void),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async (_name, callback) => {
  mocks.opened = callback;
  return mocks.dispose;
}) }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ hide: mocks.hide }) }));

afterEach(() => { cleanup(); vi.clearAllMocks(); vi.unstubAllGlobals(); mocks.opened = null; });

async function openMenu(converting = false) {
  vi.stubGlobal('__TAURI_INTERNALS__', {});
  mocks.invoke.mockResolvedValue({ language: 'en', version: '1.2.0', converting });
  const view = render(TrayMenu);
  await waitFor(() => expect(view.getByText('v1.2.0')).toBeTruthy());
  return view;
}

it('supports keyboard navigation, settings, and Escape without opening a wheel', async () => {
  const view = await openMenu();
  const settings = view.getByRole('menuitem', { name: 'Settings' });
  const quit = view.getByRole('menuitem', { name: 'Quit Vertex' });
  await waitFor(() => expect(document.activeElement).toBe(view.getByRole('menu')));
  await fireEvent.keyDown(window, { key: 'ArrowUp' });
  expect(document.activeElement).toBe(quit);
  await fireEvent.keyDown(window, { key: 'ArrowDown' });
  expect(document.activeElement).toBe(settings);
  await fireEvent.keyDown(window, { key: 'ArrowDown' });
  expect(document.activeElement).toBe(quit);
  await fireEvent.keyDown(window, { key: 'ArrowDown' });
  expect(document.activeElement).toBe(settings);
  await fireEvent.click(settings);
  expect(mocks.invoke).toHaveBeenCalledWith('tray_action', { action: 'settings' });
  await fireEvent.keyDown(window, { key: 'Escape' });
  expect(mocks.hide).toHaveBeenCalledOnce();
  mocks.opened!({ payload: { language: 'en', version: '1.2.0', converting: false } });
  await waitFor(() => expect(document.activeElement).toBe(view.getByRole('menu')));
  await fireEvent.keyDown(window, { key: 'Tab' });
  expect(document.activeElement).toBe(settings);
  view.unmount();
  expect(mocks.dispose).toHaveBeenCalledOnce();
});

it('refreshes language and conversion state each time the menu opens', async () => {
  const view = await openMenu(true);
  expect((view.getByRole('menuitem', { name: 'Settings' }) as HTMLButtonElement).disabled).toBe(true);
  expect(view.getByRole('menuitem', { name: 'Settings' }).getAttribute('title')).toBe('Converting…');
  mocks.opened!({ payload: { language: 'vi', version: '1.2.0', converting: false } });
  await waitFor(() => expect(view.getByRole('menuitem', { name: 'Cài đặt' })).toBeTruthy());
  expect((view.getByRole('menuitem', { name: 'Cài đặt' }) as HTMLButtonElement).disabled).toBe(false);
  await fireEvent.click(view.getByRole('menuitem', { name: 'Thoát Vertex' }));
  expect(mocks.invoke).toHaveBeenCalledWith('tray_action', { action: 'quit' });
});

it('keeps failed actions retryable and reports a conversion started after opening', async () => {
  const view = await openMenu();
  mocks.invoke.mockRejectedValueOnce('conversion_in_progress');
  await fireEvent.click(view.getByRole('menuitem', { name: 'Settings' }));
  await waitFor(() => expect((view.getByRole('menuitem', { name: 'Settings' }) as HTMLButtonElement).disabled).toBe(true));
  mocks.invoke.mockRejectedValueOnce('IPC unavailable');
  await fireEvent.click(view.getByRole('menuitem', { name: 'Quit Vertex' }));
  await waitFor(() => expect(view.getByText('Unable to complete. Try again.')).toBeTruthy());
  expect((view.getByRole('menuitem', { name: 'Quit Vertex' }) as HTMLButtonElement).disabled).toBe(false);
});
