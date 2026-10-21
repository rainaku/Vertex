import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { defaultOptions, normalizeOptions, SETTINGS_KEY } from './settings';
import { language } from './i18n';

const native = vi.hoisted(() => ({
  open: vi.fn(),
  startDragging: vi.fn(),
  isEnabled: vi.fn(async () => false),
  enable: vi.fn(),
  disable: vi.fn(),
}));
vi.mock('./tauri-bridge', () => ({ isTauri: true }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ startDragging: native.startDragging }) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: native.open }));
vi.mock('@tauri-apps/plugin-autostart', () => ({ isEnabled: native.isEnabled, enable: native.enable, disable: native.disable }));
vi.mock('./updater', async () => {
  const { writable } = await import('svelte/store');
  return {
    updateStore: writable({ currentVersion: '0.1.0', status: 'idle' }),
    checkForUpdates: vi.fn(), installUpdate: vi.fn(), openReleasePage: vi.fn(),
  };
});
import { checkForUpdates, installUpdate, openReleasePage, updateStore } from './updater';
import OptionsPanel from './OptionsPanel.svelte';

beforeEach(() => { language.set('en'); localStorage.clear(); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.clearAllMocks(); });

function mount(options = defaultOptions()) {
  const onSave = vi.fn();
  const onClose = vi.fn();
  return { ...render(OptionsPanel, { options, onSave, onClose }), onSave, onClose };
}

it('responds to every category and the last click during rapid navigation', async () => {
  const view = mount();
  for (const name of ['Image Quality', 'Video & Audio', 'Advanced', 'Shortcuts', 'About', 'General']) {
    await fireEvent.click(view.getByRole('button', { name }));
    expect(view.getByRole('heading', { name })).toBeTruthy();
    expect(view.getByRole('button', { name }).getAttribute('aria-current')).toBe('page');
  }
});

it('Reset changes the draft, Apply persists it, and Save persists then closes', async () => {
  const view = mount(normalizeOptions({ quality: 42, video_codec: 'h265', output_dir: 'C:\\exports' }));
  await fireEvent.click(view.getByRole('button', { name: 'Reset' }));
  expect(view.onSave).not.toHaveBeenCalled();
  await fireEvent.click(view.getByRole('button', { name: 'Apply' }));
  expect(view.onSave).toHaveBeenLastCalledWith(normalizeOptions(defaultOptions()));
  expect(JSON.parse(localStorage.getItem(SETTINGS_KEY)!)).toMatchObject(defaultOptions());
  expect(view.onClose).not.toHaveBeenCalled();
  await fireEvent.click(view.getByRole('button', { name: 'Save' }));
  expect(view.onSave).toHaveBeenCalledTimes(2);
  await waitFor(() => expect(view.onClose).toHaveBeenCalledTimes(1));
});

it.each(['Close settings', 'Escape'])('dismisses without saving via %s', async action => {
  const view = mount();
  if (action === 'Escape') await fireEvent.keyDown(window, { key: 'Escape' });
  else await fireEvent.click(view.getByRole('button', { name: action }));
  await waitFor(() => expect(view.onClose).toHaveBeenCalledTimes(1));
  expect(view.onSave).not.toHaveBeenCalled();
});

it('keeps the dialog usable and shows an error when saving fails', async () => {
  const view = mount();
  vi.spyOn(Storage.prototype, 'setItem').mockImplementationOnce(() => { throw new Error('storage unavailable'); });
  await fireEvent.click(view.getByRole('button', { name: 'Save' }));
  expect(view.getByRole('alert').textContent).toContain('Couldn’t save');
  expect(view.onClose).not.toHaveBeenCalled();
  expect(view.onSave).not.toHaveBeenCalled();
  await fireEvent.click(view.getByRole('button', { name: 'Apply' }));
  expect(view.onSave).toHaveBeenCalledTimes(1);
  expect(view.queryByRole('alert')).toBeNull();
});

it('opens project, website, and legal links through the desktop link handler', async () => {
  const view = mount();
  await fireEvent.click(view.getByRole('button', { name: 'Vertex Website' }));
  expect(openReleasePage).toHaveBeenLastCalledWith('https://rainaku.id.vn');
  await fireEvent.click(view.getByRole('button', { name: 'GitHub' }));
  expect(openReleasePage).toHaveBeenLastCalledWith('https://github.com/rainaku/Vertex');
  await fireEvent.click(view.getByRole('button', { name: 'About' }));
  for (const [name, url] of [
    ['Open GitHub', 'https://github.com/rainaku/Vertex'],
    ['Terms', 'https://github.com/rainaku/Vertex/blob/main/docs/TERMS_OF_SERVICE.md'],
    ['Privacy', 'https://github.com/rainaku/Vertex/blob/main/docs/PRIVACY_POLICY.md'],
  ]) {
    await fireEvent.click(view.getByRole('button', { name }));
    expect(openReleasePage).toHaveBeenLastCalledWith(url);
  }
});

it('uses the same folder picker and reset actions in normal view and search', async () => {
  const view = mount();
  native.open.mockResolvedValueOnce('C:\\exports');
  await fireEvent.click(view.getByRole('button', { name: 'Browse…' }));
  await waitFor(() => expect(view.getByTitle('Reset to default')).toBeTruthy());
  await fireEvent.click(view.getByTitle('Reset to default'));
  expect(view.queryByText('C:\\exports')).toBeNull();
  await fireEvent.input(view.getByRole('textbox', { name: 'Search settings' }), { target: { value: 'folder' } });
  native.open.mockResolvedValueOnce('C:\\searched');
  await fireEvent.click(view.getByRole('button', { name: 'Browse…' }));
  await waitFor(() => expect(view.getAllByText('C:\\searched')).toHaveLength(2));
  await fireEvent.click(view.getByRole('button', { name: 'Apply' }));
  expect(view.onSave).toHaveBeenLastCalledWith(expect.objectContaining({ output_dir: 'C:\\searched' }));
});

it('clicking controls never starts a native window drag', async () => {
  const view = mount();
  await fireEvent.pointerDown(view.getByRole('button', { name: 'Image Quality' }), { button: 0 });
  await fireEvent.pointerDown(view.getByRole('button', { name: 'Apply' }), { button: 0 });
  await fireEvent.pointerDown(view.getByRole('button', { name: 'Close settings' }).querySelector('path')!, { button: 0 });
  expect(native.startDragging).not.toHaveBeenCalled();
});

it('searches autostart without showing an empty result and clears search on Escape', async () => {
  const view = mount();
  await fireEvent.input(view.getByRole('textbox', { name: 'Search settings' }), { target: { value: 'startup' } });
  expect(view.getByRole('checkbox', { name: 'Start with Windows' })).toBeTruthy();
  expect(view.container.querySelector('.empty-search-card')).toBeNull();
  await fireEvent.keyDown(window, { key: 'Escape' });
  expect(view.getByRole('heading', { name: 'General' })).toBeTruthy();
  expect(view.onClose).not.toHaveBeenCalled();
});

it('toggles and persists the sound effects preference immediately', async () => {
  const view = mount();
  const checkbox = view.getByRole('checkbox', { name: 'Sound Effects' }) as HTMLInputElement;
  expect(checkbox.checked).toBe(true);
  await fireEvent.click(checkbox);
  expect(checkbox.checked).toBe(false);
  expect(localStorage.getItem('vertex.sound-enabled.v1')).toBe('false');
  await fireEvent.click(checkbox);
  expect(checkbox.checked).toBe(true);
  expect(localStorage.getItem('vertex.sound-enabled.v1')).toBe('true');
});

it('switches between Dark and Light mode and persists the theme', async () => {
  const view = mount();
  await fireEvent.click(view.getByRole('button', { name: 'Light' }));
  expect(document.documentElement.dataset.theme).toBe('light');
  expect(localStorage.getItem('vertex.theme.v1')).toBe('light');
  await fireEvent.click(view.getByRole('button', { name: 'Dark' }));
  expect(document.documentElement.dataset.theme).toBe('dark');
  expect(localStorage.getItem('vertex.theme.v1')).toBe('dark');
});

it('keeps update actions connected to their handlers', async () => {
  const view = mount();
  await fireEvent.click(view.getByRole('button', { name: 'About' }));
  await fireEvent.click(view.getByRole('button', { name: 'Check for Updates' }));
  expect(checkForUpdates).toHaveBeenCalledWith(true);
  updateStore.set({ ...get(updateStore), status: 'available' });
  await waitFor(() => expect(view.getByRole('button', { name: 'Update Now' })).toBeTruthy());
  await fireEvent.click(view.getByRole('button', { name: 'Update Now' }));
  expect(installUpdate).toHaveBeenCalledTimes(1);
});
