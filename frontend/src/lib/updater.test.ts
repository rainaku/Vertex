import { expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

vi.mock('./tauri-bridge', () => ({ isTauri: true }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.2.3' }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn(async () => null) }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn(async (_url: string) => {}) }));

it('an authoritative no-update response returns without a fallback network call', async () => {
  const fetch = vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('fallback must not run'));
  const { checkForUpdates, updateStore } = await import('./updater');
  await checkForUpdates(true);
  expect(fetch).not.toHaveBeenCalled();
  expect(get(updateStore).status).toBe('up-to-date');
  expect(get(updateStore).currentVersion).toBe('1.2.3');
  fetch.mockRestore();
});

it('opens links using the native desktop opener', async () => {
  const { openUrl } = await import('@tauri-apps/plugin-opener');
  const { openReleasePage } = await import('./updater');
  const browserOpen = vi.spyOn(window, 'open').mockImplementation(() => null);
  await openReleasePage('https://github.com/rainaku/Vertex');
  expect(openUrl).toHaveBeenCalledWith('https://github.com/rainaku/Vertex');
  expect(browserOpen).not.toHaveBeenCalled();
  browserOpen.mockRestore();
});

it('propagates native opener failures so the settings dialog can display them', async () => {
  const { openUrl } = await import('@tauri-apps/plugin-opener');
  const { openReleasePage } = await import('./updater');
  const browserOpen = vi.spyOn(window, 'open').mockImplementation(() => null);
  vi.mocked(openUrl).mockRejectedValueOnce(new Error('Could not launch browser'));
  await expect(openReleasePage('https://rainaku.id.vn')).rejects.toThrow('Could not launch browser');
  expect(browserOpen).not.toHaveBeenCalled();
  browserOpen.mockRestore();
});

it('reports a failed manual update link instead of rejecting the button handler', async () => {
  const { openUrl } = await import('@tauri-apps/plugin-opener');
  const { installUpdate, updateStore } = await import('./updater');
  updateStore.update(s => ({ ...s, directDownloadUrl: 'https://github.com/rainaku/Vertex/releases/latest' }));
  vi.mocked(openUrl).mockRejectedValueOnce(new Error('Could not launch browser'));
  await expect(installUpdate()).resolves.toBeUndefined();
  expect(get(updateStore).status).toBe('error');
  expect(get(updateStore).error).toBe('Could not launch browser');
});
