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

export async function detectFile(path: string): Promise<FormatInfo> {
  if (!isTauri) {
    // Browser mock for dev
    const ext = path.split('.').pop()?.toUpperCase() || 'PNG';
    let cat = 'image';
    if (['MP3', 'WAV', 'FLAC', 'AAC', 'OGG', 'M4A', 'OPUS'].includes(ext)) {
      cat = 'audio';
    } else if (['MP4', 'MOV', 'MKV', 'AVI', 'WEBM', 'WMV', 'FLV'].includes(ext)) {
      cat = 'video';
    }
    return {
      format: ext,
      label: ext,
      extension: ext.toLowerCase(),
      category: cat,
      filename: path.split(/[/\\]/).pop() || `test.${ext.toLowerCase()}`,
    };
  }
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<FormatInfo>('detect_file', { path });
}

export async function getAvailableTargets(from: string): Promise<TargetFormatInfo[]> {
  if (!isTauri) {
    // Browser mock defaults
    const audio = ['MP3', 'WAV', 'FLAC', 'AAC', 'OGG', 'M4A', 'OPUS'];
    const video = ['MP4', 'WEBM', 'MOV', 'MKV', 'AVI', 'GIF', ...audio];
    const image = ['PNG', 'JPG', 'WEBP', 'AVIF', 'BMP', 'ICO', 'TIFF', 'GIF', 'PDF'];

    let pool = image;
    if (audio.includes(from)) {
      pool = audio;
    } else if (video.includes(from)) {
      pool = video;
    }

    return pool
      .filter((f) => f !== from)
      .map((f) => {
        let cat = 'image';
        if (audio.includes(f)) cat = 'audio';
        else if (['MP4', 'WEBM', 'MOV', 'MKV', 'AVI', 'WMV', 'FLV', '3GP', 'TS'].includes(f)) cat = 'video';
        return {
          format: f,
          label: f,
          extension: f.toLowerCase(),
          category: cat,
          available: true,
        };
      });
  }
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<TargetFormatInfo[]>('get_available_targets', { from });
}

export async function getBatchInfo(paths: string[]): Promise<BatchInfo> {
  if (!isTauri) {
    const files: FormatInfo[] = paths.map((p) => {
      const ext = p.split('.').pop()?.toUpperCase() || 'PNG';
      return {
        format: ext,
        label: ext,
        extension: ext.toLowerCase(),
        category: 'image',
        filename: p.split(/[/\\]/).pop() || 'file',
      };
    });
    return {
      files,
      common_targets: [
        { format: 'PNG', label: 'PNG', extension: 'png', category: 'image', available: true },
        { format: 'WEBP', label: 'WEBP', extension: 'webp', category: 'image', available: true },
        { format: 'JPG', label: 'JPG', extension: 'jpg', category: 'image', available: true },
        { format: 'AVIF', label: 'AVIF', extension: 'avif', category: 'image', available: true },
      ],
    };
  }
  const { invoke } = await import('@tauri-apps/api/core');
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
          success: true,
          output_path: inputPath.replace(/\.[^.]+$/, `.${targetFormat.toLowerCase()}`),
          target_format: targetFormat,
        });
      }, 1200);
    });
  }

  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<ConvertResult>('convert_file', {
    jobId,
    inputPath,
    targetFormat,
    options,
  });
}

export async function cancelJob(jobId: string): Promise<boolean> {
  if (!isTauri) return true;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<boolean>('cancel_job', { jobId });
}

export async function revealInExplorer(path: string): Promise<void> {
  if (!isTauri) {
    console.log('Reveal in explorer mock:', path);
    return;
  }
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('reveal_in_explorer', { path });
}

export async function hideWheelWindow(): Promise<void> {
  if (!isTauri) return;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('hide_wheel_window');
}

export async function showWheelWindow(): Promise<void> {
  if (!isTauri) return;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('show_wheel_window');
}

/**
 * Toggle OS-level cursor passthrough on the transparent window.
 * passthrough=true  → window is invisible to mouse clicks (click-through to desktop)
 * passthrough=false → window captures mouse, wheel becomes interactive
 */
export async function setWindowPassthrough(passthrough: boolean): Promise<void> {
  if (!isTauri) return;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('set_window_passthrough', { passthrough });
}

export async function setConvertingState(converting: boolean): Promise<void> {
  if (!isTauri) return;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('set_converting_state', { converting });
}


export async function pickFile(): Promise<string | null> {
  if (!isTauri) {
    return new Promise((resolve) => {
      const input = document.createElement('input');
      input.type = 'file';
      input.onchange = () => {
        const file = input.files?.[0];
        resolve(file ? file.name : null);
      };
      input.click();
    });
  }
  const { open } = await import('@tauri-apps/plugin-dialog');
  const selected = await open({
    multiple: false,
    filters: [
      {
        name: 'Supported Files',
        extensions: [
          'png', 'jpg', 'jpeg', 'webp', 'avif', 'bmp', 'ico', 'tiff', 'gif',
          'pdf', 'docx', 'txt', 'md', 'html', 'csv', 'json', 'xlsx'
        ],
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
