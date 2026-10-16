import type { ConvertResult } from './types';

/** Snapshot + bounded sequential dispatch; cancellation also stops pending files. */
export async function runConversionBatch(paths: readonly string[], hooks: {
  convert: (path: string, index: number) => Promise<ConvertResult>;
  stopped: () => boolean;
  completed: (count: number) => void;
}) {
  const outputs: string[] = [];
  const failures: { path: string; error: string }[] = [];
  let cancelled = false;
  for (const [index, path] of [...paths].entries()) {
    if (hooks.stopped()) { cancelled = true; break; }
    try {
      const result = await hooks.convert(path, index);
      if (result.cancelled) { cancelled = true; break; }
      if (result.success) outputs.push(result.output_path);
      else failures.push({ path, error: result.error || 'Conversion failed' });
    } catch (error) {
      failures.push({ path, error: String(error) });
    }
    hooks.completed(index + 1);
  }
  return { outputs, failures, cancelled };
}
