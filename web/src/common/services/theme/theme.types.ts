/* Тема, которая сейчас применена. */
export type TResolvedTheme = 'light' | 'dark';

/* Значение контекста темы. */
export interface IThemeContextValue {
  resolved: TResolvedTheme;
}
