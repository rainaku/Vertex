import { writable } from 'svelte/store';

export type Theme = 'dark' | 'light';

export const THEME_KEY = 'vertex.theme.v1';

function initialTheme(): Theme {
  try {
    const saved = localStorage.getItem(THEME_KEY);
    if (saved === 'light' || saved === 'dark') return saved;
  } catch {}
  return 'dark';
}

export const theme = writable<Theme>(initialTheme());

export function applyTheme(value: Theme): void {
  if (typeof document !== 'undefined' && document.documentElement.dataset.theme !== value) {
    document.documentElement.dataset.theme = value;
  }
}

export function setTheme(value: Theme): void {
  theme.set(value);
  try {
    localStorage.setItem(THEME_KEY, value);
  } catch {}
}

theme.subscribe((value) => {
  applyTheme(value);
});

if (typeof window !== 'undefined') {
  window.addEventListener('storage', (e) => {
    if (e.key === THEME_KEY && (e.newValue === 'light' || e.newValue === 'dark')) {
      theme.set(e.newValue);
    }
  });
}

