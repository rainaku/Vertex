import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/svelte';
import { hitTestClient } from './geometry';
import { normalizeOptions } from './settings';
import { sliderValue } from './slider';
import { runConversionBatch } from './conversion-batch';
import RadialWheel from './RadialWheel.svelte';
import OptionsPanel from './OptionsPanel.svelte';
import { compareVersions } from './updater';

afterEach(cleanup);

describe('audit regressions', () => {
  it('normalizes transformed pointer coordinates across wheel scale and DPI', () => {
    for (const scale of [0.35, 0.55, 1, 1.5]) {
      const rect = { left: 170, top: 90, width: 520 * scale, height: 520 * scale };
      expect(hitTestClient(rect.left + 260 * scale, rect.top + 260 * scale, rect).type).toBe('center');
      for (let i = 0; i < 8; i++) {
        const angle = -Math.PI / 2 + (i + 0.5) * Math.PI / 4;
        expect(hitTestClient(rect.left + (260 + 150 * Math.cos(angle)) * scale,
          rect.top + (260 + 150 * Math.sin(angle)) * scale, rect)).toEqual({ type: 'petal', index: i });
      }
    }
  });

  it('retains DPI and metadata choices while rejecting invalid numeric options', () => {
    const options = normalizeOptions({ dpi: 320, strip_metadata: false, max_width: Infinity, quality: NaN });
    expect(options.dpi).toBe(320);
    expect(options.strip_metadata).toBe(false);
    expect(options.max_width).toBeUndefined();
    expect(options.quality).toBe(85);
  });

  it('snaps slider steps from min and permits the upper endpoint', () => {
    expect(sliderValue(82, 72, 600, 10)).toBe(82);
    expect(sliderValue(600, 72, 600, 10)).toBe(600);
    expect(sliderValue(5, 5, 5, 0)).toBe(5);
  });

  it('orders prereleases using SemVer precedence', () => {
    expect(compareVersions('0.1.0-alpha.1', '0.1.0')).toBe(-1);
    expect(compareVersions('0.1.0-alpha.10', '0.1.0-alpha.2')).toBe(1);
    expect(compareVersions('v0.1.0+build.42', '0.1.0')).toBe(0);
  });

  it('converts every batch member and retains failures without abandoning later files', async () => {
    const convert = vi.fn(async (path: string) => ({ cancelled: false, success: path !== 'bad', output_path: path + '.png', target_format: 'PNG', error: 'bad input' }));
    const completed = vi.fn();
    const result = await runConversionBatch(['first', 'bad', 'last'], { convert, stopped: () => false, completed });
    expect(convert.mock.calls.map(([path]) => path)).toEqual(['first', 'bad', 'last']);
    expect(result.outputs).toEqual(['first.png', 'last.png']);
    expect(result.failures).toHaveLength(1);
    expect(completed).toHaveBeenLastCalledWith(3);
  });

  it('cancels pending batch members', async () => {
    let stopped = false;
    const convert = vi.fn(async () => { stopped = true; return { cancelled: true, success: false, output_path: '', target_format: 'PNG' }; });
    const result = await runConversionBatch(['one', 'two'], { convert, stopped: () => stopped, completed: () => {} });
    expect(result.cancelled).toBe(true);
    expect(convert).toHaveBeenCalledTimes(1);
  });

  it('keyboard selection passes the actual petal index without pointer hover', async () => {
    const onSelectFormat = vi.fn();
    const fmt = { format: 'PNG', label: 'PNG', extension: 'png', category: 'image', available: true };
    const view = render(RadialWheel, { formats: [fmt], activeIndex: -1, onSelectFormat });
    await fireEvent.keyDown(view.getByRole('button', { name: 'PNG' }), { key: 'Enter' });
    expect(onSelectFormat).toHaveBeenCalledWith(fmt, 0);
  });

  it('search renders preset/shortcut controls and updates as the query changes', async () => {
    const view = render(OptionsPanel, { options: normalizeOptions({}), onSave: () => {}, onClose: () => {} });
    const search = view.container.querySelector('input.search-input')!;
    await fireEvent.input(search, { target: { value: 'preset' } });
    expect(view.getByRole('button', { name: 'Fast' })).toBeTruthy();
    await fireEvent.input(search, { target: { value: 'shortcut' } });
    expect(view.getByText('Shift + Drag')).toBeTruthy();
    expect(view.queryByRole('button', { name: 'Fast' })).toBeNull();
    await fireEvent.input(search, { target: { value: 'no-match-123' } });
    expect(view.container.querySelector('.empty-search-card')).toBeTruthy();
  });
});
