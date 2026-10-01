import { writable } from 'svelte/store';
import { isTauri } from './tauri-bridge';

export interface UpdateInfo {
  status: 'idle' | 'checking' | 'up-to-date' | 'available' | 'downloading' | 'downloaded' | 'error';
  currentVersion: string;
  latestVersion: string;
  releaseNotes: string;
  publishedAt: string;
  downloadProgress: number; // 0 to 100
  bytesDownloaded: number;
  totalBytes: number;
  error: string | null;
  directDownloadUrl: string | null;
}

const GITHUB_REPO = 'rainaku/Vertex';
const APP_VERSION = '0.1.0';

export const updateStore = writable<UpdateInfo>({
  status: 'idle',
  currentVersion: APP_VERSION,
  latestVersion: APP_VERSION,
  releaseNotes: '',
  publishedAt: '',
  downloadProgress: 0,
  bytesDownloaded: 0,
  totalBytes: 0,
  error: null,
  directDownloadUrl: null,
});

let activeUpdateInstance: any = null;

/**
 * Compare two semver strings (e.g. "0.1.1" vs "0.1.0").
 * Returns 1 if vA > vB, -1 if vA < vB, 0 if equal.
 */
export function compareVersions(vA: string, vB: string): number {
  const parse = (v: string) =>
    v.replace(/^[^\d]*/, '').split('.').map((n) => parseInt(n, 10) || 0);
  const a = parse(vA);
  const b = parse(vB);
  const len = Math.max(a.length, b.length);
  for (let i = 0; i < len; i++) {
    const numA = a[i] ?? 0;
    const numB = b[i] ?? 0;
    if (numA > numB) return 1;
    if (numA < numB) return -1;
  }
  return 0;
}

/**
 * Check for new releases.
 * 1. Attempts Tauri official updater plugin (`check()`).
 * 2. Fallbacks to GitHub Releases API if plugin check yields no update or errors.
 */
export async function checkForUpdates(manual: boolean = false): Promise<void> {
  updateStore.update((s) => ({
    ...s,
    status: 'checking',
    error: null,
  }));

  // 1. Try Tauri v2 official Updater Plugin
  if (isTauri) {
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const update = await check();
      if (update) {
        activeUpdateInstance = update;
        updateStore.set({
          status: 'available',
          currentVersion: update.currentVersion || APP_VERSION,
          latestVersion: update.version,
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
    } catch (err: any) {
      console.warn('[Updater] Tauri plugin check failed, falling back to GitHub API:', err);
    }
  }

  // 2. Fallback to GitHub Releases API
  try {
    const res = await fetch(`https://api.github.com/repos/${GITHUB_REPO}/releases/latest`, {
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
      ? data.assets.find((a: any) => a.name?.endsWith('.exe'))
      : null;
    const downloadUrl = exeAsset ? exeAsset.browser_download_url : data.html_url;

    if (tag && compareVersions(tag, APP_VERSION) > 0) {
      updateStore.set({
        status: 'available',
        currentVersion: APP_VERSION,
        latestVersion: tag,
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
        error: null,
      }));
    }
  } catch (err: any) {
    console.error('[Updater] Check failed:', err);
    updateStore.update((s) => ({
      ...s,
      status: manual ? 'error' : 'up-to-date',
      error: manual ? (err.message || 'Không thể kết nối đến máy chủ cập nhật.') : null,
    }));
  }
}

/**
 * Downloads and installs the pending update.
 */
export async function installUpdate(): Promise<void> {
  if (!activeUpdateInstance) {
    // If we only have direct download URL, open it
    let targetUrl: string | null = null;
    updateStore.subscribe((s) => {
      targetUrl = s.directDownloadUrl;
    })();
    if (targetUrl) {
      await openReleasePage(targetUrl);
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

    await activeUpdateInstance.downloadAndInstall((event: any) => {
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
  } catch (err: any) {
    console.error('[Updater] Install failed:', err);
    updateStore.update((s) => ({
      ...s,
      status: 'error',
      error: `Lỗi cài đặt: ${err.message || err}`,
    }));
  }
}

/**
 * Open GitHub release page in the default web browser.
 */
export async function openReleasePage(customUrl?: string): Promise<void> {
  const url = customUrl || `https://github.com/${GITHUB_REPO}/releases/latest`;
  if (isTauri) {
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(url);
      return;
    } catch {}
  }
  window.open(url, '_blank', 'noopener,noreferrer');
}
