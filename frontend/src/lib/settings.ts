import type { Options } from './types';

export const SETTINGS_KEY = 'vertex.conversion-options.v1';

export function defaultOptions(): Options {
  return {
    quality: 85, dpi: 200, strip_metadata: true,
    collision_policy: 'rename_with_suffix',
    jpeg_background: [255, 255, 255], gif_alpha_threshold: 128, avif_speed: 6,
    video_codec: 'h264', video_preset: 'medium', video_crf: 23, video_audio_kbps: 128,
  };
}

export function normalizeOptions(value: Partial<Options> | null | undefined): Options {
  const defaults = defaultOptions();
  const src = value ?? {};
  const integer = (n: unknown, min: number, max: number, fallback: number) =>
    typeof n === 'number' && Number.isFinite(n) ? Math.min(max, Math.max(min, Math.round(n))) : fallback;
  const dimension = (n: unknown) =>
    typeof n === 'number' && Number.isFinite(n) && n > 0 ? integer(n, 1, 32768, 1) : undefined;
  return {
    ...defaults,
    quality: integer(src.quality, 1, 100, defaults.quality),
    dpi: integer(src.dpi, 72, 600, defaults.dpi),
    strip_metadata: typeof src.strip_metadata === 'boolean' ? src.strip_metadata : defaults.strip_metadata,
    max_width: dimension(src.max_width),
    max_height: dimension(src.max_height),
    output_dir: typeof src.output_dir === 'string' ? src.output_dir.trim() || undefined : undefined,
    collision_policy: ['rename_with_suffix', 'fail_if_exists', 'overwrite'].includes(src.collision_policy ?? '')
      ? src.collision_policy!
      : defaults.collision_policy,
    jpeg_background:
      Array.isArray(src.jpeg_background) && src.jpeg_background.length === 3
        ? (src.jpeg_background.map((n) => integer(n, 0, 255, 255)) as [number, number, number])
        : defaults.jpeg_background,
    gif_alpha_threshold: integer(src.gif_alpha_threshold, 1, 255, defaults.gif_alpha_threshold ?? 128),
    avif_speed: integer(src.avif_speed, 1, 10, defaults.avif_speed ?? 6),
    video_codec: src.video_codec === 'h265' ? 'h265' : 'h264',
    video_preset: src.video_preset === 'fast' || src.video_preset === 'slow' ? src.video_preset : 'medium',
    video_crf: integer(src.video_crf, 18, 35, defaults.video_crf ?? 23),
    video_audio_kbps: integer(src.video_audio_kbps, 64, 320, defaults.video_audio_kbps ?? 128),
  };
}

export function loadOptions(): Options {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    const parsed = raw ? JSON.parse(raw) : {};
    return normalizeOptions(parsed && typeof parsed === 'object' ? parsed : {});
  } catch {
    return defaultOptions();
  }
}

export function saveOptions(value: Partial<Options>): Options {
  const normalized = normalizeOptions(value);
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(normalized));
  } catch {
    // Ignore storage quota or private mode errors
  }
  return normalized;
}
