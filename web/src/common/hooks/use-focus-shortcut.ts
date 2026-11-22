import { useEffect, useRef } from 'react';

import { isFocusShortcut } from 'common/utils/dom';

/** «/» или Cmd/Ctrl+K — вызвать `onShortcut` (обычно фокус на поле поиска). */
export function useFocusShortcut(onShortcut: () => void): void {
  const handler = useRef(onShortcut);
  useEffect(() => {
    handler.current = onShortcut;
  });
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (!isFocusShortcut(e)) return;
      e.preventDefault();
      handler.current();
    }
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);
}
