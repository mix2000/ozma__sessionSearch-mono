import { useCallback, useSyncExternalStore } from 'react';

/** Совпадает ли media query; следим за изменением на лету. */
export function useMediaQuery(query: string): boolean {
  const subscribe = useCallback(
    function subscribe(onChange: () => void) {
      const mql = window.matchMedia?.(query);
      mql?.addEventListener('change', onChange);
      return () => mql?.removeEventListener('change', onChange);
    },
    [query]
  );
  const getSnapshot = useCallback(
    function getSnapshot() {
      return window.matchMedia?.(query).matches ?? false;
    },
    [query]
  );
  return useSyncExternalStore(subscribe, getSnapshot, () => false);
}
