import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useSearchState } from './use-search-state';

describe('useSearchState', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    window.history.replaceState(null, '', '/');
  });

  it('начальное состояние — из адреса', () => {
    window.history.replaceState(null, '', '/?q=nginx&provider=codex&headless=1&x=1');
    const { result } = renderHook(() => useSearchState());
    expect(result.current).toMatchObject({
      input: 'nginx',
      q: 'nginx',
      provider: 'codex',
      includeHeadless: true,
    });
  });

  it('ввод попадает в запрос через задержку, Enter — сразу; всё — в адрес', () => {
    window.history.replaceState(null, '', '/?x=1#h');
    const { result } = renderHook(() => useSearchState());
    act(() => result.current.setInput('деплой '));
    expect(result.current.q).toBe('');
    act(() => vi.advanceTimersByTime(400));
    expect(result.current.q).toBe('деплой');
    expect(window.location.search).toBe('?x=1&q=%D0%B4%D0%B5%D0%BF%D0%BB%D0%BE%D0%B9');
    expect(window.location.hash).toBe('#h');

    act(() => result.current.submit(' nginx '));
    expect(result.current.q).toBe('nginx');
    act(() => {
      result.current.setProvider('grok');
      result.current.setIncludeHeadless(true);
    });
    expect(window.location.search).toBe('?x=1&q=nginx&provider=grok&headless=1');
  });
});
