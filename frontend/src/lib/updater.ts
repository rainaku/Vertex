import { writable, get } from 'svelte/store';
import { clean, coerce, compare, valid } from 'semver';
import type { DownloadEvent, Update } from '@tauri-apps/plugin-updater';
import { isTauri } from './tauri-bridge';
import { t } from './i18n';

interface GitHubReleaseAsset {
  name?: string;
  browser_download_url?: string;
}

export interface UpdateInfo {
  status: 'idle' | 'checking' | 'up-to-date' | 'available' | 'downloading' | 'downloaded' | 'error';
  currentVersion: string;
  latestVersion: string;
  version?: string | null;
  releaseNotes: string;
  publishedAt: string;
  downloadProgress: number; // 0 to 100
  bytesDownloaded: number;
  totalBytes: number;
  error: string | null;
  directDownloadUrl: string | null;
}

const GITHUB_REPO = 'rainaku/Vertex';
const APP_VERSION = '1.2.2';

export const updateStore = writable<UpdateInfo>({
  status: 'idle',
  currentVersion: APP_VERSION,
  latestVersion: APP_VERSION,
  version: null,
  releaseNotes: '',
  publishedAt: '',
  downloadProgress: 0,
  bytesDownloaded: 0,
  totalBytes: 0,
  error: null,
  directDownloadUrl: null,
});

let activeUpdateInstance: Update | null = null;
let checkInFlight: Promise<void> | null = null;

export function checkForUpdates(manual = false): Promise<void> {
  if (checkInFlight) return checkInFlight;
  if (get(updateStore).status === 'downloading') return Promise.resolve();
  checkInFlight = performUpdateCheck(manual).finally(() => { checkInFlight = null; });
  return checkInFlight;
}

/**
 * Compare two semver strings (e.g. "0.1.1" vs "0.1.0").
 * Returns 1 if vA > vB, -1 if vA < vB, 0 if equal.
 */
export function compareVersions(a: string, b: string): number {
  const normA = valid(clean(a)) ?? coerce(a)?.version ?? null;
  const normB = valid(clean(b)) ?? coerce(b)?.version ?? null;
  if (!normA || !normB) return 0;
  return compare(normA, normB);
}
/**
 * Check for new releases.
 * 1. Attempts Tauri official updater plugin (`check()`).
 * 2. Falls back to GitHub Releases API only when the plugin check fails.
 */
async function performUpdateCheck(manual: boolean = false): Promise<void> {
  updateStore.update((s) => ({
    ...s,
    status: 'checking',
    error: null,
  }));

  // 1. Try Tauri v2 official Updater Plugin
  if (isTauri) {
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const { getVersion } = await import('@tauri-apps/api/app');
      const currentVersion = await getVersion();
      updateStore.update(s => ({ ...s, currentVersion }));
      await activeUpdateInstance?.close();
      activeUpdateInstance = null;
      const update = await check();
      if (!update) {
        updateStore.update(s => ({ ...s, status: 'up-to-date', latestVersion: currentVersion, version: null, error: null }));
        return;
      }
      if (update) {
        activeUpdateInstance = update;
        updateStore.set({
          status: 'available',
          currentVersion: update.currentVersion || APP_VERSION,
          latestVersion: update.version,
          version: update.version,
          releaseNotes: update.body || '',
          publishedAt: update.date || '',
          downloadProgress: 0,
          bytesDownloaded: 0,
          totalBytes: 0,
          error: null,
          directDownloadUrl: null,
        });
        return;
      }
    } catch (err: unknown) {
      console.warn('[Updater] Tauri plugin check failed, falling back to GitHub API:', err);
    }
  }

  // 2. Fallback to GitHub Releases API
  try {
    const res = await fetch(`https://api.github.com/repos/${GITHUB_REPO}/releases/latest`, {
      signal: AbortSignal.timeout(15000),
      headers: {
        Accept: 'application/vnd.github.v3+json',
      },
    });

    if (res.status === 404) {
      // No releases published yet on GitHub repo
      updateStore.update((s) => ({
        ...s,
        status: 'up-to-date',
        latestVersion: s.currentVersion,
        error: null,
      }));
      return;
    }

    if (!res.ok) {
      throw new Error(`GitHub API HTTP ${res.status}: ${res.statusText}`);
    }

    const data = await res.json();
    const tag = (data.tag_name || '').replace(/^v/, '');
    const notes = data.body || '';
    const pubDate = data.published_at || '';

    // Find Windows installer asset (.exe)
    const exeAsset = Array.isArray(data.assets)
      ? data.assets.find((a: GitHubReleaseAsset) => a.name?.endsWith('.exe'))
      : null;
    const downloadUrl = exeAsset ? exeAsset.browser_download_url : data.html_url;

    const currentVersion = get(updateStore).currentVersion;
    if (tag && compareVersions(tag, currentVersion) > 0) {
      updateStore.set({
        status: 'available',
        currentVersion,
        latestVersion: tag,
        version: tag,
        releaseNotes: notes,
        publishedAt: pubDate,
        downloadProgress: 0,
        bytesDownloaded: 0,
        totalBytes: 0,
        error: null,
        directDownloadUrl: downloadUrl,
      });
    } else {
      updateStore.update((s) => ({
        ...s,
        status: 'up-to-date',
        latestVersion: tag || s.currentVersion,
        version: null,
        error: null,
      }));
    }
  } catch (err: unknown) {
    console.error('[Updater] Check failed:', err);
    const errMsg = err instanceof Error && err.message ? err.message : get(t)('Không thể kết nối đến máy chủ cập nhật.');
    updateStore.update((s) => ({
      ...s,
      status: manual ? 'error' : 'idle',
      error: manual ? errMsg : null,
    }));
  }
}

/**
 * Downloads and installs the pending update.
 */
export async function installUpdate(): Promise<void> {
  const currentStatus = get(updateStore).status;
  if (currentStatus === 'downloading') return;
  if (currentStatus === 'downloaded') {
    if (isTauri) {
      try {
        const { relaunch } = await import('@tauri-apps/plugin-process');
        await relaunch();
      } catch {}
    }
    return;
  }

  if (!activeUpdateInstance) {
    // If we only have direct download URL, open it
    const targetUrl = get(updateStore).directDownloadUrl;
    if (targetUrl) {
      try {
        await openReleasePage(targetUrl);
      } catch (err: unknown) {
        updateStore.update(s => ({
          ...s,
          status: 'error',
          error: err instanceof Error ? err.message : String(err),
        }));
      }
    }
    return;
  }

  updateStore.update((s) => ({
    ...s,
    status: 'downloading',
    downloadProgress: 0,
    bytesDownloaded: 0,
    totalBytes: 0,
    error: null,
  }));

  try {
    let totalLen = 0;
    let downloaded = 0;

    await activeUpdateInstance.downloadAndInstall((event: DownloadEvent) => {
      if (event.event === 'Started') {
        totalLen = event.data?.contentLength || 0;
        updateStore.update((s) => ({
          ...s,
          totalBytes: totalLen,
        }));
      } else if (event.event === 'Progress') {
        downloaded += event.data?.chunkLength || 0;
        const progress = totalLen > 0 ? Math.min(100, Math.round((downloaded / totalLen) * 100)) : 0;
        updateStore.update((s) => ({
          ...s,
          bytesDownloaded: downloaded,
          downloadProgress: progress,
        }));
      } else if (event.event === 'Finished') {
        updateStore.update((s) => ({
          ...s,
          status: 'downloaded',
          downloadProgress: 100,
        }));
      }
    });

    // On Windows, downloadAndInstall launches the installer and exits.
    // On other platforms, relaunch() will restart into the new version.
    if (isTauri) {
      try {
        const { relaunch } = await import('@tauri-apps/plugin-process');
        await relaunch();
      } catch {}
    }
  } catch (err: unknown) {
    console.error('[Updater] Install failed:', err);
    const detail = err instanceof Error && err.message ? err.message : String(err);
    updateStore.update((s) => ({
      ...s,
      status: 'error',
      error: `${get(t)('Lỗi cài đặt')}: ${detail}`,
    }));
  }
}

/**
 * Open GitHub release page in the default web browser.
 */
export async function openReleasePage(customUrl?: string): Promise<void> {
  const url = customUrl || `https://github.com/${GITHUB_REPO}/releases/latest`;
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error(get(t)('Không thể kết nối đến máy chủ cập nhật.'));
  }
  if (parsed.protocol !== 'https:') {
    throw new Error(get(t)('Không thể kết nối đến máy chủ cập nhật.'));
  }
  if (isTauri) {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
    return;
  }
  window.open(url, '_blank', 'noopener,noreferrer');
}
