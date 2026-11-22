import { fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from 'common/services/api';
import { makeResult, makeStatus, renderWithProviders } from 'common/test/test-utils';

import { TranscriptsSearch } from './search';

const api = vi.hoisted(() => ({
  search: vi.fn(),
  status: vi.fn(),
  sync: vi.fn(),
  context: vi.fn(),
  resume: vi.fn(),
}));
vi.mock('entities/transcripts/transcripts.api', () => ({ transcriptsApi: api }));

function openAt(url: string) {
  window.history.replaceState(null, '', url);
  return renderWithProviders(<TranscriptsSearch />);
}

describe('TranscriptsSearch', () => {
  beforeEach(() => {
    Object.values(api).forEach(fn => fn.mockReset());
    api.status.mockResolvedValue(makeStatus());
  });
  afterEach(() => window.history.replaceState(null, '', '/'));

  it('поиск из адреса → карточки', async () => {
    api.search.mockResolvedValue({ terms: ['nginx'], results: [makeResult()], truncated: false });
    openAt('/?q=nginx');
    expect(await screen.findByText('Разговоров: 1')).toBeInTheDocument();
    expect(screen.getByText('Починка nginx')).toBeInTheDocument();
  });

  it('контекст открывается и закрывается', async () => {
    api.search.mockResolvedValue({ terms: ['nginx'], results: [makeResult()], truncated: false });
    api.context.mockResolvedValue([
      { id: 1, seq: 4, role: 'user', at: null, text: 'почини nginx полностью' },
    ]);
    openAt('/?q=nginx');
    fireEvent.click(await screen.findByRole('button', { name: 'Контекст' }));
    const full = (_: string, el: Element | null) =>
      el?.tagName === 'PRE' && el.textContent === 'почини nginx полностью';
    expect(await screen.findByText(full)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    // Цель сброшена: antd запускает анимацию закрытия (в jsdom она не доигрывает).
    await waitFor(() => expect(document.querySelector('.ant-modal')?.className).toMatch(/leave/));
  });

  it('Enter ищет сразу; ничего не найдено', async () => {
    api.search.mockResolvedValue({ terms: ['zzz'], results: [], truncated: false });
    openAt('/');
    const input = screen.getByRole('searchbox');
    fireEvent.change(input, { target: { value: 'zzz' } });
    fireEvent.keyDown(input, { key: 'Enter', code: 'Enter', keyCode: 13 });
    expect(await screen.findByText('Ничего не найдено')).toBeInTheDocument();
    expect(api.search).toHaveBeenCalledWith({ q: 'zzz', provider: '', includeHeadless: false });
  });

  it('короткий запрос — подсказка с порогом из статуса, без запроса', async () => {
    api.status.mockResolvedValue(makeStatus({ minQueryLength: 3 }));
    openAt('/?q=ab');
    expect(await screen.findByText('Введите хотя бы 3 символа')).toBeInTheDocument();
    expect(api.search).not.toHaveBeenCalled();
  });

  it('пока статус не загружен — поиск не запускается', async () => {
    api.status.mockReturnValue(new Promise(() => {}));
    openAt('/?q=nginx');
    await waitFor(() => expect(api.status).toHaveBeenCalled());
    expect(api.search).not.toHaveBeenCalled();
    expect(screen.queryByText(/Введите хотя бы/)).toBeNull();
  });

  it('ошибка поиска — текст сервера', async () => {
    api.search.mockRejectedValue(new ApiError(400, 'плохой запрос'));
    openAt('/?q=nginx');
    expect(await screen.findByText('плохой запрос')).toBeInTheDocument();
  });

  it('сбой поиска без ответа сервера — «Не удалось выполнить поиск»', async () => {
    api.search.mockRejectedValue(new Error('boom'));
    openAt('/?q=nginx');
    expect(await screen.findByText('Не удалось выполнить поиск')).toBeInTheDocument();
    expect(screen.queryByText('Ничего не найдено')).toBeNull();
  });
});
