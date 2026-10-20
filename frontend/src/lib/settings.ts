import type { Options } from './types';
import contract from './conversion-contract.json';

const limits = contract.limits;

export const SETTINGS_KEY = 'vertex.conversion-options.v1';

export function defaultOptions(): Options {
  const { max_width: _width, max_height: _height, output_dir: _directory, ...defaults } = structuredClone(contract.default_options);
  return defaults as Options;
}

export function normalizeOptions(value: Partial<Options> | null | undefined): Options {
  const defaults = defaultOptions();
  const src = value ?? {};
  const integer = (n: unknown, min: number, max: number, fallback: number) =>
    typeof n === 'number' && Number.isFinite(n) ? Math.min(max, Math.max(min, Math.round(n))) : fallback;
  const dimension = (key: 'max_width' | 'max_height', n: unknown) =>
    typeof n === 'number' && Number.isFinite(n) && n > 0 ? integer(n, limits[key].min, limits[key].max, 1) : undefined;
  const bounded = (key: keyof typeof limits, n: unknown, fallback: number) => integer(n, limits[key].min, limits[key].max, fallback);
  return {
    ...defaults,
    quality: bounded('quality', src.quality, defaults.quality),
    dpi: bounded('dpi', src.dpi, defaults.dpi),
    strip_metadata: typeof src.strip_metadata === 'boolean' ? src.strip_metadata : defaults.strip_metadata,
    max_width: dimension('max_width', src.max_width),
    max_height: dimension('max_height', src.max_height),
    output_dir: typeof src.output_dir === 'string' && src.output_dir.trim() ? src.output_dir.trim() : undefined,
    collision_policy: ['rename_with_suffix', 'fail_if_exists', 'overwrite'].includes(src.collision_policy ?? '')
      ? src.collision_policy!
      : defaults.collision_policy,
    jpeg_background:
      Array.isArray(src.jpeg_background) && src.jpeg_background.length === 3
        ? (src.jpeg_background.map((n) => integer(n, 0, 255, 255)) as [number, number, number])
        : defaults.jpeg_background,
    gif_alpha_threshold: bounded('gif_alpha_threshold', src.gif_alpha_threshold, defaults.gif_alpha_threshold!),
    avif_speed: bounded('avif_speed', src.avif_speed, defaults.avif_speed!),
    video_codec: src.video_codec === 'h265' ? 'h265' : 'h264',
    video_preset: src.video_preset === 'fast' || src.video_preset === 'slow' ? src.video_preset : 'medium',
    video_crf: bounded('video_crf', src.video_crf, defaults.video_crf!),
    video_audio_kbps: bounded('video_audio_kbps', src.video_audio_kbps, defaults.video_audio_kbps!),
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
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(normalized));
  return normalized;
}
