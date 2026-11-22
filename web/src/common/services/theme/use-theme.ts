import { useContext } from 'react';

import { ThemeContext } from './theme.context';

import type { IThemeContextValue } from './theme.types';

/** Текущая тема приложения. */
export function useTheme(): IThemeContextValue {
  return useContext(ThemeContext);
}
