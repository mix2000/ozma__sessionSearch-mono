import { act, fireEvent, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockMatchMedia } from 'common/test/test-utils';

import { useDebouncedValue } from './use-debounced-value';
import { useDocumentTitle } from './use-document-title';
import { useFocusShortcut } from './use-focus-shortcut';
import { useIsTouch } from './use-is-touch';
import { useMediaQuery } from './use-media-query';
import { usePrefersDark } from './use-prefers-dark';

afterEach(() => {
  vi.useRealTimers();
});

describe('useDebouncedValue', () => {
  it('отдаёт значение через задержку после последнего изменения', () => {
    vi.useFakeTimers();
    const { result, rerender } = renderHook(({ v }) => useDebouncedValue(v, 400), {
      initialProps: { v: 'a' },
    });
    rerender({ v: 'ab' });
    act(() => vi.advanceTimersByTime(300));
    rerender({ v: 'abc' });
    act(() => vi.advanceTimersByTime(300));
    expect(result.current).toBe('a');
    act(() => vi.advanceTimersByTime(100));
    expect(result.current).toBe('abc');
  });
});

describe('useDocumentTitle', () => {
  it('ставит заголовок вкладки', () => {
    renderHook(() => useDocumentTitle('Заголовок'));
    expect(document.title).toBe('Заголовок');
  });
});

describe('useFocusShortcut', () => {
  it('«/» и Ctrl+K вызывают обработчик, ввод в поле — нет', () => {
    const onShortcut = vi.fn();
    const { unmount } = renderHook(() => useFocusShortcut(onShortcut));
    fireEvent.keyDown(window, { key: '/' });
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    fireEvent.keyDown(window, { key: 'x' });
    const input = document.createElement('input');
    document.body.append(input);
    fireEvent.keyDown(input, { key: '/' });
    expect(onShortcut).toHaveBeenCalledTimes(2);
    unmount();
    fireEvent.keyDown(window, { key: '/' });
    expect(onShortcut).toHaveBeenCalledTimes(2);
    input.remove();
  });
});

describe('media query hooks', () => {
  it('следят за изменением и отписываются', () => {
    const media = mockMatchMedia(['(pointer: coarse)']);
    const touch = renderHook(() => useIsTouch());
    const dark = renderHook(() => usePrefersDark());
    expect(touch.result.current).toBe(true);
    expect(dark.result.current).toBe(false);
    act(() => media.set('(prefers-color-scheme: dark)', true));
    expect(dark.result.current).toBe(true);
    const q = renderHook(() => useMediaQuery('(min-width: 1px)'));
    expect(q.result.current).toBe(false);
    expect(media.listenerCount('(min-width: 1px)')).toBe(1);
    q.unmount();
    expect(media.listenerCount('(min-width: 1px)')).toBe(0);
  });
});
