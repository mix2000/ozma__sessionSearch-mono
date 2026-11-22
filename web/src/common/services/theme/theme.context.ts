import { createContext } from 'react';

import type { IThemeContextValue } from './theme.types';

/* Вне провайдера — светлая: экран не падает без темы. */
export const ThemeContext = createContext<IThemeContextValue>({ resolved: 'light' });
