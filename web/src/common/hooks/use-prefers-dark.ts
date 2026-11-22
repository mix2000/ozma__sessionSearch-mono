import { useMediaQuery } from './use-media-query';

const QUERY = '(prefers-color-scheme: dark)';

/** Тёмная тема системы — следим за переключением на лету. */
export function usePrefersDark(): boolean {
  return useMediaQuery(QUERY);
}
