import { afterEach, expect, it, vi } from 'vitest';
import contract from './conversion-contract.json';

const native = vi.hoisted(() => ({ invoke: vi.fn(), open: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: native.open }));

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.resetModules();
  vi.clearAllMocks();
});

it('desktop picker gets supported extensions from the backend', async () => {
  Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {}, configurable: true });
  vi.resetModules();
  const { pickFile } = await import('./tauri-bridge');
  // A deliberately different catalogue proves the picker is not using its
  // browser snapshot or a second hardcoded extension list.
  native.invoke.mockResolvedValueOnce([{ format: 'TEST', extensions: ['one', 'two'] }]);
  native.open.mockResolvedValueOnce('C:\\source.one');
  await expect(pickFile()).resolves.toBe('C:\\source.one');
  expect(native.invoke).toHaveBeenCalledWith('get_input_formats');
  expect(native.open).toHaveBeenCalledWith(expect.objectContaining({ filters: [expect.objectContaining({ extensions: ['one', 'two'] })] }));
});

it('normalizes options before invoking the conversion command without damaging output paths', async () => {
  Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {}, configurable: true });
  vi.resetModules();
  const { convertFile } = await import('./tauri-bridge');
  native.invoke.mockResolvedValueOnce({ success: true });
  await convertFile('job', 'C:\\source.pdf', 'PNG', { dpi: 1000, video_audio_kbps: 999, output_dir: 'file:///C:/d%E1%BB%AF%20li%E1%BB%87u' });
  expect(native.invoke).toHaveBeenCalledWith('convert_file', expect.objectContaining({
    options: expect.objectContaining({ dpi: contract.limits.dpi.max, video_audio_kbps: contract.limits.video_audio_kbps.max, output_dir: 'file:///C:/d%E1%BB%AF%20li%E1%BB%87u' }),
  }));
});
