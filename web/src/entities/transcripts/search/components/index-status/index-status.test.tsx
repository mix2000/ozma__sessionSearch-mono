import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from 'common/services/api';
import { makeStatus, renderWithProviders } from 'common/test/test-utils';

import { IndexStatus } from './index-status';

const api = vi.hoisted(() => ({ status: vi.fn(), sync: vi.fn() }));
vi.mock('entities/transcripts/transcripts.api', () => ({ transcriptsApi: api }));

describe('IndexStatus', () => {
  beforeEach(() => {
    api.status.mockReset();
    api.sync.mockReset();
  });

  it('счётчики, ошибка индекса, источники', async () => {
    api.status.mockResolvedValue(
      makeStatus({
        index: { running: true, lastRunAt: null, lastDurationMs: null, lastError: 'диск' },
      })
    );
    renderWithProviders(<IndexStatus />);
    expect(
      await screen.findByText('В индексе 3 разговоров, 10 реплик · индексирую…')
    ).toBeInTheDocument();
    expect(screen.getByText('диск')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Источники/ }));
    expect(await screen.findByText('Какие папки индексируются')).toBeInTheDocument();
    expect(screen.getByText('/c')).toBeInTheDocument();
  });

  it('сервер не отвечает — текст ошибки, без источников', async () => {
    api.status.mockRejectedValue(new ApiError(0, 'Сервер поиска не отвечает'));
    renderWithProviders(<IndexStatus />);
    expect(await screen.findByText('Сервер поиска не отвечает')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Источники/ })).toBeNull();
  });

  it('«Обновить индекс»: успех и ошибка — сообщением', async () => {
    api.status.mockResolvedValue(makeStatus({ providers: [] }));
    api.sync.mockResolvedValueOnce({ files: 2, updated: 2, messages: 5 });
    api.sync.mockResolvedValueOnce({ files: 2, updated: 0, messages: 0 });
    api.sync.mockRejectedValueOnce(new ApiError(500, 'индекс занят'));
    renderWithProviders(<IndexStatus />);
    const button = await screen.findByRole('button', { name: /Обновить индекс/ });
    fireEvent.click(button);
    expect(await screen.findByText('Обновлено разговоров: 2')).toBeInTheDocument();
    fireEvent.click(button);
    expect(await screen.findByText('Новых реплик нет')).toBeInTheDocument();
    fireEvent.click(button);
    expect(await screen.findByText('индекс занят')).toBeInTheDocument();
    await waitFor(() => expect(api.sync).toHaveBeenCalledTimes(3));
  });
});
