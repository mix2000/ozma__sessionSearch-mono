/* vitest: матчеры jest-dom, заглушки браузерных API, которых нет в jsdom,
 * и чистка DOM после каждого теста (как в CheerPortal). */
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

import '@testing-library/jest-dom/vitest';

/* antd и наши хуки спрашивают matchMedia — отвечаем «не совпало» (десктоп,
 * светлая тема). Тест может подменить window.matchMedia своим. */
if (typeof window !== 'undefined' && typeof window.matchMedia !== 'function') {
  window.matchMedia = (query: string): MediaQueryList =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList;
}

/* antd следит за размерами через ResizeObserver — в jsdom его нет. */
if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  } as unknown as typeof ResizeObserver;
}

afterEach(() => {
  cleanup();
});
