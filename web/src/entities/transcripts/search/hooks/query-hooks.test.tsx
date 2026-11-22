import { act, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { makeStatus, renderHookWithProviders } from 'common/test/test-utils';

import { useResumeTranscript } from './use-resume-transcript';
import { useSyncTranscripts } from './use-sync-transcripts';
import { useTranscriptSearch } from './use-transcript-search';
import { useTranscriptStatus } from './use-transcript-status';

const api = vi.hoisted(() => ({
  search: vi.fn(),
  status: vi.fn(),
  sync: vi.fn(),
  context: vi.fn(),
  resume: vi.fn(),
}));
vi.mock('entities/transcripts/transcripts.api', () => ({ transcriptsApi: api }));

const response = { terms: ['nginx'], results: [], truncated: false };

describe('query hooks', () => {
  beforeEach(() => {
    Object.values(api).forEach(fn => fn.mockReset());
  });

  it('useTranscriptSearch: короткий запрос не уходит', () => {
    const short = renderHookWithProviders(() => useTranscriptSearch({ q: ' a ' }, 2));
    expect(short.result.current.fetchStatus).toBe('idle');
    expect(api.search).not.toHaveBeenCalled();
  });

  it('useTranscriptSearch: порог ещё неизвестен — запрос не уходит', () => {
    const r = renderHookWithProviders(() => useTranscriptSearch({ q: 'nginx' }));
    expect(r.result.current.fetchStatus).toBe('idle');
  });

  it('useTranscriptSearch: запрос с фильтрами', async () => {
    api.search.mockResolvedValue(response);
    const ok = renderHookWithProviders(() =>
      useTranscriptSearch({ q: 'nginx', provider: 'codex', includeHeadless: true }, 2)
    );
    await waitFor(() => expect(ok.result.current.data).toEqual(response));
    expect(api.search).toHaveBeenCalledTimes(1);
    expect(api.search).toHaveBeenCalledWith({
      q: 'nginx',
      provider: 'codex',
      includeHeadless: true,
    });
  });

  it('useTranscriptStatus', async () => {
    api.status.mockResolvedValue(makeStatus());
    const { result } = renderHookWithProviders(() => useTranscriptStatus());
    await waitFor(() => expect(result.current.data?.sessions).toBe(3));
  });

  it('useSyncTranscripts и useResumeTranscript инвалидируют всё про разговоры', async () => {
    api.sync.mockResolvedValue({ files: 1, updated: 1, messages: 2 });
    api.resume.mockResolvedValue({ ok: true, command: 'cd /w && claude --resume x' });
    const sync = renderHookWithProviders(() => useSyncTranscripts());
    const spy = vi.spyOn(sync.client, 'invalidateQueries');
    await act(() => sync.result.current.mutateAsync());
    expect(spy).toHaveBeenCalledWith({ queryKey: ['transcripts'] });

    const resume = renderHookWithProviders(() => useResumeTranscript());
    const spy2 = vi.spyOn(resume.client, 'invalidateQueries');
    await expect(act(() => resume.result.current.mutateAsync('x'))).resolves.toMatchObject({
      ok: true,
    });
    expect(api.resume).toHaveBeenCalledWith('x');
    expect(spy2).toHaveBeenCalledWith({ queryKey: ['transcripts'] });
  });
});
