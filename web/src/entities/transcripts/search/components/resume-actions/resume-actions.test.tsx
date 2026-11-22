import { fireEvent, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from 'common/services/api';
import { makeResult, renderWithProviders } from 'common/test/test-utils';

import { ResumeActions } from './resume-actions';

const api = vi.hoisted(() => ({ resume: vi.fn() }));
vi.mock('entities/transcripts/transcripts.api', () => ({ transcriptsApi: api }));

const COMMAND = 'cd /Users/demo/proj && claude --resume aaaa-1111';

describe('ResumeActions', () => {
  beforeEach(() => {
    api.resume.mockReset();
  });

  it('«Продолжить»: успех — сообщение', async () => {
    api.resume.mockResolvedValue({ ok: true, command: COMMAND });
    renderWithProviders(<ResumeActions result={makeResult({ resumeCommand: COMMAND })} />);
    fireEvent.click(screen.getByRole('button', { name: /Продолжить/ }));
    expect(await screen.findByText('Открыто в терминале')).toBeInTheDocument();
    expect(api.resume).toHaveBeenCalledWith('aaaa-1111');
  });

  it('«Продолжить»: ошибка — текст сервера', async () => {
    api.resume.mockRejectedValue(new ApiError(409, 'Разговор уже открыт в терминале (pid 1)'));
    renderWithProviders(<ResumeActions result={makeResult()} />);
    fireEvent.click(screen.getByRole('button', { name: /Продолжить/ }));
    expect(await screen.findByText('Разговор уже открыт в терминале (pid 1)')).toBeInTheDocument();
  });

  it('«Продолжить»: сбой без ответа сервера — запасной текст', async () => {
    api.resume.mockRejectedValue(new Error('boom'));
    renderWithProviders(<ResumeActions result={makeResult()} />);
    fireEvent.click(screen.getByRole('button', { name: /Продолжить/ }));
    expect(await screen.findByText('Не удалось продолжить')).toBeInTheDocument();
  });

  it('уже открыт в терминале — без кнопки', () => {
    renderWithProviders(
      <ResumeActions result={makeResult({ live: { kind: 'external', pid: 3 } })} />
    );
    expect(screen.getByText('Уже открыт в терминале')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Продолжить/ })).toBeNull();
  });

  it('нет папки — без кнопки, команда от бэкенда остаётся', () => {
    renderWithProviders(
      <ResumeActions
        result={makeResult({
          provider: 'grok',
          workdirExists: false,
          resumeCommand: 'cd /w && grok -s x',
        })}
      />
    );
    expect(screen.getByText('Папки разговора уже нет — продолжить нельзя')).toBeInTheDocument();
    expect(screen.getByText('cd /w && grok -s x')).toBeInTheDocument();
  });

  it('без команды от бэкенда — блока команды нет', () => {
    const { container } = renderWithProviders(<ResumeActions result={makeResult()} />);
    expect(container.querySelector('code')).toBeNull();
  });
});
