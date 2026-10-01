import type { Options } from './types';

export const SETTINGS_KEY = 'vertex.conversion-options.v1';

export function defaultOptions(): Options {
  return {
    quality: 85, dpi: 200, strip_metadata: true,
    collision_policy: 'rename_with_suffix',
    jpeg_background: [255, 255, 255], gif_alpha_threshold: 128, avif_speed: 6,
  };
}

export function normalizeOptions(value: Partial<Options>): Options {
  const defaults = defaultOptions();
  const integer = (n: unknown, min: number, max: number, fallback: number) =>
    typeof n === 'number' && Number.isFinite(n) ? Math.min(max, Math.max(min, Math.round(n))) : fallback;
  const dimension = (n: unknown) => typeof n === 'number' && n > 0 ? integer(n, 1, 32768, 1) : undefined;
  return {
    ...defaults,
    quality: integer(value.quality, 1, 100, defaults.quality),
    max_width: dimension(value.max_width), max_height: dimension(value.max_height),
    output_dir: typeof value.output_dir === 'string' ? value.output_dir.trim() || undefined : undefined,
    collision_policy: ['rename_with_suffix', 'fail_if_exists', 'overwrite'].includes(value.collision_policy ?? '')
      ? value.collision_policy! : defaults.collision_policy,
    jpeg_background: Array.isArray(value.jpeg_background) && value.jpeg_background.length === 3
      ? value.jpeg_background.map(n => integer(n, 0, 255, 255)) as [number, number, number]
      : defaults.jpeg_background,
    gif_alpha_threshold: integer(value.gif_alpha_threshold, 1, 255, 128),
    avif_speed: integer(value.avif_speed, 1, 10, 6),
  };
}

export function loadOptions(): Options {
  try {
    return normalizeOptions(JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? '{}') ?? {});
  } catch {
    return defaultOptions();
  }
}
