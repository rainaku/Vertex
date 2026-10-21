import contract from './conversion-contract.json';
import { normalizeOptions } from './settings';
import { invoke } from '@tauri-apps/api/core';
import { translate } from './i18n';
import type {
  FormatInfo,
  TargetFormatInfo,
  BatchInfo,
  ProgressPayload,
  ConvertResult,
  Options,
} from './types';

// Check if running inside Tauri runtime
export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const FORMAT_BY_ID = new Map(contract.formats.map((item) => [item.format, item]));
const FORMAT_BY_EXT = new Map(
  contract.formats.flatMap((item) => item.extensions.map((ext) => [ext.toLowerCase(), item] as const))
);

export async function getInputFormats() {
  if (!isTauri) return contract.formats;
  return invoke<typeof contract.formats>('get_input_formats');
}

export function mockOutputPath(path: string, target: string): string {
  const boundary = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1;
  const directory = path.slice(0, boundary);
  const name = path.slice(boundary);
  const stem = /\.tar\.gz$/i.test(name) ? name.slice(0, -7) : name.replace(/\.[^.]+$/, '');
  const extension = FORMAT_BY_ID.get(target)?.extension ?? target.toLowerCase();
  return directory + stem + '.' + extension;
}

export async function detectFile(path: string): Promise<FormatInfo> {
  if (!isTauri) {
    const filename = path.split(/[/\\]/).pop() || path;
    const lower = filename.toLowerCase();
    let definition: (typeof contract.formats)[number] | undefined;
    let dot = lower.indexOf('.');
    while (dot !== -1) {
      definition = FORMAT_BY_EXT.get(lower.slice(dot + 1));
      if (definition) break;
      dot = lower.indexOf('.', dot + 1);
    }
    if (!definition) throw new Error('Unsupported input format: ' + filename);
    return { ...definition, filename };
  }
  return invoke<FormatInfo>('detect_file', { path });
}

export async function getAvailableTargets(from: string): Promise<TargetFormatInfo[]> {
  if (!isTauri) {
    const source = FORMAT_BY_ID.get(from);
    const notices = source?.notices as Record<string, { vi: string; en: string }> | undefined;
    return (source?.targets ?? []).map(format => {
      const item = FORMAT_BY_ID.get(format);
      if (!item) throw new Error('Missing format metadata: ' + format);
      const notice = notices?.[format];
      return { format, label: item.label, extension: item.extension, category: item.category, available: true, notices: notice ? [notice] : [] };
    });
  }
  return invoke<TargetFormatInfo[]>('get_available_targets', { from });
}

export async function getBatchInfo(paths: string[]): Promise<BatchInfo> {
  if (!isTauri) {
    const files = await Promise.all(paths.map(detectFile));
    const targets = await Promise.all(files.map(file => getAvailableTargets(file.format)));
    const targetMaps = targets.map(list => new Map(list.map(item => [item.format, item])));
    return {
      files,
      common_targets: (targets[0] ?? []).filter(target =>
        targetMaps.every(map => map.has(target.format))).map(target => ({
          ...target,
          notices: [...new Map(targetMaps.flatMap(map => map.get(target.format)?.notices ?? []).map(notice => [notice.en, notice])).values()],
        })),
    };
  }
  return invoke<BatchInfo>('get_batch_info', { paths });
}

export async function convertFile(
  jobId: string,
  inputPath: string,
  targetFormat: string,
  options?: Partial<Options>
): Promise<ConvertResult> {
  if (!isTauri) {
    // Simulated progress in browser
    return new Promise((resolve) => {
      setTimeout(() => {
        resolve({
          cancelled: false,
          success: true,
          output_path: mockOutputPath(inputPath, targetFormat),
          target_format: targetFormat,
        });
      }, 1200);
    });
  }

  return invoke<ConvertResult>('convert_file', {
    jobId,
    inputPath,
    targetFormat,
    options: options ? normalizeOptions(options) : undefined,
  });
}

export async function cancelJob(jobId: string): Promise<boolean> {
  if (!isTauri) return true;
  return invoke<boolean>('cancel_job', { jobId });
}

export async function revealInExplorer(path: string): Promise<void> {
  if (!isTauri) {
    console.log('Reveal in explorer mock:', path);
    return;
  }
  return invoke('reveal_in_explorer', { path });
}

export async function hideWheelWindow(generation: number): Promise<void> {
  if (!isTauri) return;
  return invoke('hide_wheel_window', { generation });
}

export async function forceHideWheelWindow(): Promise<void> {
  if (!isTauri) return;
  return invoke('force_hide_wheel_window');
}

/**
 * Ensure native input capture before interacting with the wheel.
 * Legacy passthrough=true requests are ignored; hidden native windows already
 * let the desktop receive input without changing WebView2 hit-test flags.
 */
export async function setWindowPassthrough(passthrough: boolean): Promise<void> {
  if (!isTauri) return;
  return invoke('set_window_passthrough', { passthrough });
}

export async function setConvertingState(converting: boolean): Promise<void> {
  if (!isTauri) return;
  return invoke('set_converting_state', { converting });
}


export async function pickFile(): Promise<string | null> {
  if (!isTauri) {
    return new Promise((resolve) => {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = contract.formats.flatMap(format => format.extensions.map(ext => '.' + ext)).join(',');
      input.onchange = () => {
        const file = input.files?.[0];
        resolve(file ? file.name : null);
      };
      input.oncancel = () => resolve(null);
      input.click();
    });
  }
  const { open } = await import('@tauri-apps/plugin-dialog');
  const selected = await open({
    multiple: false,
    filters: [
      {
        name: translate('Tệp được hỗ trợ'),
        extensions: (await getInputFormats()).flatMap(format => format.extensions),
      },
    ],
  });
  if (typeof selected === 'string') return selected;
  return null;
}

export async function sendDesktopNotification(title: string, body: string): Promise<void> {
  if (!isTauri) {
    if ('Notification' in window && Notification.permission === 'granted') {
      new Notification(title, { body });
    }
    return;
  }
  try {
    const { isPermissionGranted, requestPermission, sendNotification } = await import(
      '@tauri-apps/plugin-notification'
    );
    let granted = await isPermissionGranted();
    if (!granted) {
      const perm = await requestPermission();
      granted = perm === 'granted';
    }
    if (granted) {
      sendNotification({ title, body });
    }
  } catch (e) {
    console.warn('Failed to send notification:', e);
  }
}

export async function listenProgress(
  callback: (payload: ProgressPayload) => void
): Promise<() => void> {
  if (!isTauri) {
    return () => {};
  }
  const { listen } = await import('@tauri-apps/api/event');
  return listen<ProgressPayload>('convert_progress', (event) => {
    callback(event.payload);
  });
}

export async function centerWindow(): Promise<void> {
  if (!isTauri) return;
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().center();
  } catch {
      await invoke('center_window');
  }
}

export async function prepareSettingsWindow(): Promise<void> {
  if (!isTauri) return;
  await invoke('prepare_settings_window');
}

export async function restoreWheelWindow(): Promise<void> {
  if (!isTauri) return;
  await invoke('restore_wheel_window');
}
