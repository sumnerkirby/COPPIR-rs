// Themes are sets of CSS design tokens on :root. Every component colours
// itself from these tokens, so switching theme restyles the whole app.

import type { Branding } from './types/Branding';
import type { Theme } from './types/Theme';

type Tokens = Record<'bg' | 'panel' | 'panel-2' | 'line' | 'text' | 'muted' | 'accent' | 'accent-ink' | 'danger' | 'warn', string>;

export const THEMES: Record<Theme, { label: string; light: boolean; tokens: Tokens }> = {
  terminal: {
    label: 'Terminal',
    light: false,
    tokens: {
      bg: '#050a06', panel: '#0b130d', 'panel-2': '#101b13', line: '#1d3022', text: '#d6f5dc', muted: '#6f8f76',
      accent: '#00ff41', 'accent-ink': '#001a06', danger: '#ff4d4d', warn: '#ffaa00',
    },
  },
  midnight: {
    label: 'Midnight',
    light: false,
    tokens: {
      bg: '#0a0f1c', panel: '#111827', 'panel-2': '#172036', line: '#26324d', text: '#e3e9f5', muted: '#8a96b0',
      accent: '#5aa9ff', 'accent-ink': '#04121f', danger: '#ff5c5c', warn: '#ffb020',
    },
  },
  daylight: {
    label: 'Daylight',
    light: true,
    tokens: {
      bg: '#eef1f4', panel: '#ffffff', 'panel-2': '#f5f7f9', line: '#d3dae1', text: '#14202b', muted: '#5b6876',
      accent: '#0a7d32', 'accent-ink': '#ffffff', danger: '#c62828', warn: '#a35f00',
    },
  },
  contrast: {
    label: 'High contrast',
    light: false,
    tokens: {
      bg: '#000000', panel: '#000000', 'panel-2': '#121212', line: '#ffffff', text: '#ffffff', muted: '#e0e0e0',
      accent: '#ffff00', 'accent-ink': '#000000', danger: '#ff6b6b', warn: '#ffd000',
    },
  },
};

/** Black or white, whichever reads better on `hex`. */
export function inkFor(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  if (Number.isNaN(n)) return '#000000';
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? '#000000' : '#ffffff';
}

export function tokensFor(b: Pick<Branding, 'theme' | 'accent'>): Tokens {
  const t = { ...THEMES[b.theme].tokens };
  if (b.accent) {
    t.accent = b.accent;
    t['accent-ink'] = inkFor(b.accent);
  }
  return t;
}

export function applyTheme(b: Pick<Branding, 'theme' | 'accent'>) {
  const root = document.documentElement;
  for (const [k, v] of Object.entries(tokensFor(b))) root.style.setProperty(`--${k}`, v);
  root.style.colorScheme = THEMES[b.theme].light ? 'light' : 'dark';
}
