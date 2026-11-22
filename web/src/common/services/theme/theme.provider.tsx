import { ConfigProvider } from 'antd';
import ruRU from 'antd/locale/ru_RU';
import { useEffect, useMemo } from 'react';

import { usePrefersDark } from 'common/hooks';

import { ThemeContext } from './theme.context';
import { antdThemeFor, applyTheme, resolveTheme } from './theme-dom';

import type { ReactNode } from 'react';

interface IThemeProviderProps {
  children: ReactNode;
}

/** Светлая или тёмная тема по системной настройке; antd и `data-theme` на `<html>`. */
export function ThemeProvider({ children }: IThemeProviderProps) {
  const resolved = resolveTheme(usePrefersDark());

  useEffect(() => {
    applyTheme(resolved);
  }, [resolved]);

  const value = useMemo(() => ({ resolved }), [resolved]);

  return (
    <ThemeContext.Provider value={value}>
      <ConfigProvider locale={ruRU} theme={antdThemeFor(resolved)}>
        {children}
      </ConfigProvider>
    </ThemeContext.Provider>
  );
}
