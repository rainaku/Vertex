import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/svelte';
import contract from './conversion-contract.json';
import { defaultOptions, normalizeOptions } from './settings';
import { language } from './i18n';
import OptionsPanel from './OptionsPanel.svelte';
import RadialWheel from './RadialWheel.svelte';

afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

it('uses backend defaults and bounds for every numeric conversion option', () => {
  expect(normalizeOptions(contract.default_options as Parameters<typeof normalizeOptions>[0])).toEqual(normalizeOptions(defaultOptions()));
  for (const [key, { min, max }] of Object.entries(contract.limits)) {
    const name = key as keyof typeof contract.limits;
    expect(normalizeOptions({ [name]: max + 1 })[name]).toBe(max);
    expect(normalizeOptions({ [name]: min })[name]).toBe(min);
  }
  const a = defaultOptions();
  a.jpeg_background![0] = 0;
  expect(defaultOptions().jpeg_background).toEqual([255, 255, 255]);
});

it('browser format detection and target discovery use the registry snapshot', async () => {
  const bridge = await import('./tauri-bridge');
  for (const definition of contract.formats) {
    for (const alias of definition.extensions) {
      expect((await bridge.detectFile('C:\\jane.doe\\FILE.' + alias.toUpperCase())).format).toBe(definition.format);
    }
    const targets = await bridge.getAvailableTargets(definition.format);
    expect(targets.map(item => item.format)).toEqual(definition.targets);
    for (const target of targets) expect(target.category).toBe(contract.formats.find(item => item.format === target.format)?.category);
  }
  expect(bridge.mockOutputPath('C:\\jane.doe\\Makefile', 'PNG')).toBe('C:\\jane.doe\\Makefile.png');
  expect(bridge.mockOutputPath('C:\\data\\sample.TAR.GZ', '7Z')).toBe('C:\\data\\sample.7z');
  await expect(bridge.detectFile('unsupported.unknown')).rejects.toThrow('Unsupported input format');
});

it('video audio bitrate is editable in its tab and in search with the same bounds', async () => {
  language.set('en');
  const onSave = vi.fn();
  const view = render(OptionsPanel, { options: defaultOptions(), onSave, onClose: () => {} });
  await fireEvent.click(view.getByRole('button', { name: 'Video & Audio' }));
  let slider = view.getByRole('slider', { name: 'Video audio bitrate (kbps)' });
  expect(slider.getAttribute('aria-valuemax')).toBe(String(contract.limits.video_audio_kbps.max));
  expect(view.queryByText('AVIF encoding speed')).toBeNull();
  expect(view.getByRole('button', { name: 'Fast' })).toBeTruthy();
  await fireEvent.keyDown(slider, { key: 'End' });
  await fireEvent.click(view.getByRole('button', { name: /^Appl/ }));
  expect(onSave).toHaveBeenLastCalledWith(expect.objectContaining({ video_audio_kbps: 320 }));
  await fireEvent.input(view.getByRole('textbox', { name: 'Search settings' }), { target: { value: 'kbps' } });
  slider = view.getByRole('slider', { name: 'Video audio bitrate (kbps)' });
  await fireEvent.keyDown(slider, { key: 'Home' });
  await fireEvent.click(view.getByRole('button', { name: /^Appl/ }));
  expect(onSave).toHaveBeenLastCalledWith(expect.objectContaining({ video_audio_kbps: 64 }));
});

it('image options stay in the image tab and searching does not recreate the pane', async () => {
  language.set('en');
  const view = render(OptionsPanel, { options: defaultOptions(), onSave: () => {}, onClose: () => {} });
  await fireEvent.click(view.getByRole('button', { name: 'Image Quality' }));
  expect(view.getByText('AVIF encoding speed')).toBeTruthy();
  expect(view.getByText('GIF alpha threshold')).toBeTruthy();
  const pane = view.container.querySelector('.content-scroll');
  await fireEvent.input(view.getByRole('textbox', { name: 'Search settings' }), { target: { value: 'dpi' } });
  expect(view.container.querySelector('.content-scroll')).toBe(pane);
  expect(view.container.querySelector('.card .divider')).toBeNull();
  expect(view.getByText('PDF / SVG render DPI')).toBeTruthy();
});

it('shows actual engine requirements and route limitations in the wheel', async () => {
  language.set('en');
  const missing = { format: 'DOCX', label: 'DOCX', extension: 'docx', category: 'document', available: false, reason: 'LibreOffice is required' };
  const view = render(RadialWheel, { formats: [missing], activeIndex: 0 });
  expect(view.getByText('LibreOffice is required')).toBeTruthy();
  await view.rerender({ formats: [{ ...missing, available: true, notices: [{ vi: 'Trang đầu', en: 'First page only' }] }], activeIndex: 0 });
  expect(view.getByRole('note').textContent).toContain('First page only');
});
