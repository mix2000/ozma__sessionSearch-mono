// Применение темы к документу. Тема следует системной (prefers-color-scheme),
// выбор нигде не хранится.

import { darkTheme, lightTheme } from './antd-theme';

import type { ThemeConfig } from 'antd';
import type { TResolvedTheme } from './theme.types';

/** Атрибут на `<html>`, по которому ветвятся стили (global.scss). */
export const THEME_ATTRIBUTE = 'theme';

export function resolveTheme(prefersDark: boolean): TResolvedTheme {
  return prefersDark ? 'dark' : 'light';
}

/** `<html data-theme="…">`. */
export function applyTheme(
  resolved: TResolvedTheme,
  root: HTMLElement = document.documentElement
): void {
  root.dataset[THEME_ATTRIBUTE] = resolved;
}

/** Тема antd для применённой темы. */
export function antdThemeFor(resolved: TResolvedTheme): ThemeConfig {
  return resolved === 'dark' ? darkTheme : lightTheme;
}
