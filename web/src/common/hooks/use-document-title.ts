import { useEffect } from 'react';

/** Заголовок вкладки браузера. */
export function useDocumentTitle(title: string): void {
  useEffect(() => {
    document.title = title;
  }, [title]);
}
