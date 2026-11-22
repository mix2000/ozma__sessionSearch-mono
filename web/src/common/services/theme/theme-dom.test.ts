import { describe, expect, it } from 'vitest';

import { darkTheme, lightTheme } from './antd-theme';
import { antdThemeFor, applyTheme, resolveTheme } from './theme-dom';

describe('theme-dom', () => {
  it('системная тёмная → dark', () => {
    expect(resolveTheme(true)).toBe('dark');
    expect(resolveTheme(false)).toBe('light');
  });

  it('data-theme на корне и тема antd', () => {
    const root = document.createElement('html');
    applyTheme('dark', root);
    expect(root.dataset.theme).toBe('dark');
    applyTheme('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(antdThemeFor('dark')).toBe(darkTheme);
    expect(antdThemeFor('light')).toBe(lightTheme);
  });
});
