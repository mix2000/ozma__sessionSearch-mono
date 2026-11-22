import { fireEvent, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from 'common/services/api';
import { makeResult, renderWithProviders } from 'common/test/test-utils';

import { ContextModal } from './context-modal';

const api = vi.hoisted(() => ({ context: vi.fn() }));
vi.mock('entities/transcripts/transcripts.api', () => ({ transcriptsApi: api }));

const messages = [
  { id: 1, seq: 3, role: 'user', at: '2026-11-10T08:55:00.000Z', text: 'почини nginx' },
  { id: 2, seq: 4, role: 'assistant', at: null, text: 'Смотрю конфиг nginx.' },
];

describe('ContextModal', () => {
  beforeEach(() => {
    api.context.mockReset();
  });

  it('закрыта без цели, запрос не идёт', () => {
    renderWithProviders(<ContextModal target={null} terms={[]} onClose={() => {}} />);
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(api.context).not.toHaveBeenCalled();
  });

  it('реплики с подсветкой, текущая выделена', async () => {
    api.context.mockResolvedValue(messages);
    const target = { result: makeResult(), seq: 4 };
    const onClose = vi.fn();
    renderWithProviders(<ContextModal target={target} terms={['nginx']} onClose={onClose} />);
    expect(await screen.findByText('Починка nginx')).toBeInTheDocument();
    const current = (await screen.findByText(/Смотрю конфиг/)).closest('div[class*="contextMsg"]');
    expect(current?.className).toContain('contextCurrent');
    expect(document.querySelectorAll('.ant-modal mark')).toHaveLength(2);
    expect(api.context).toHaveBeenCalledWith('aaaa-1111', 4);
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalled();
  });

  it('ошибка загрузки; заголовок по умолчанию', async () => {
    api.context.mockRejectedValue(new ApiError(500, ''));
    const target = { result: makeResult({ title: null }), seq: 1 };
    renderWithProviders(<ContextModal target={target} terms={[]} onClose={() => {}} />);
    expect(await screen.findByText('Не удалось загрузить контекст')).toBeInTheDocument();
    expect(screen.getByText('Разговор вокруг совпадения')).toBeInTheDocument();
  });
});
