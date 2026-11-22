import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { mockMatchMedia } from 'common/test/test-utils';

import { SearchToolbar } from './search-toolbar';

import type { ISearchController } from '../../models';

function setup(over: Partial<ISearchController> = {}) {
  const state: ISearchController = {
    input: '',
    setInput: vi.fn(),
    submit: vi.fn(),
    q: '',
    provider: '',
    setProvider: vi.fn(),
    includeHeadless: false,
    setIncludeHeadless: vi.fn(),
    ...over,
  };
  render(<SearchToolbar state={state} loading={false} />);
  return state;
}

describe('SearchToolbar', () => {
  it('ввод, Enter, автоматические', () => {
    const p = setup({ input: 'ngi' });
    const input = screen.getByRole('searchbox', { name: /Что искать/ });
    expect(input).toHaveValue('ngi');
    fireEvent.change(input, { target: { value: 'nginx' } });
    expect(p.setInput).toHaveBeenCalledWith('nginx');
    fireEvent.keyDown(input, { key: 'Enter', code: 'Enter', keyCode: 13 });
    expect(p.submit).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('checkbox', { name: /Включая автоматические/ }));
    expect(p.setIncludeHeadless).toHaveBeenCalledWith(true);
  });

  it('выбор агента (нативный список на тач-устройстве)', () => {
    mockMatchMedia(['(pointer: coarse)']);
    const p = setup();
    fireEvent.change(screen.getByRole('combobox', { name: 'Все агенты' }), {
      target: { value: 'grok' },
    });
    expect(p.setProvider).toHaveBeenCalledWith('grok');
    expect(screen.getAllByRole('option').map(o => o.textContent)).toEqual([
      'Все агенты',
      'Claude',
      'Codex',
      'Grok',
    ]);
  });

  it('«/» ставит фокус в поле поиска', () => {
    mockMatchMedia();
    setup();
    const input = screen.getByRole('searchbox');
    input.blur();
    fireEvent.keyDown(document.body, { key: '/' });
    expect(input).toHaveFocus();
  });
});
