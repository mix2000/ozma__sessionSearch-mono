import { beforeEach, describe, expect, it, vi } from 'vitest';

import { transcriptsApi } from './transcripts.api';

const api = vi.hoisted(() => ({ get: vi.fn(), post: vi.fn() }));
vi.mock('common/services/api', () => ({ apiService: api }));

describe('transcriptsApi', () => {
  beforeEach(() => {
    api.get.mockReset().mockResolvedValue({});
    api.post.mockReset().mockResolvedValue({});
  });

  it('search: пустые фильтры не уходят, headless — строкой', async () => {
    await transcriptsApi.search({ q: 'nginx', provider: '', includeHeadless: false });
    await transcriptsApi.search({ q: 'x', provider: 'codex', includeHeadless: true });
    expect(api.get).toHaveBeenNthCalledWith(1, '/agents/transcripts/search', {
      q: 'nginx',
      provider: undefined,
      includeHeadless: undefined,
    });
    expect(api.get).toHaveBeenNthCalledWith(2, '/agents/transcripts/search', {
      q: 'x',
      provider: 'codex',
      includeHeadless: 'true',
    });
  });

  it('status, sync, context, resume', async () => {
    await transcriptsApi.status();
    await transcriptsApi.sync();
    await transcriptsApi.context('a b', 5);
    await transcriptsApi.resume('a/b');
    expect(api.get).toHaveBeenCalledWith('/agents/transcripts/status');
    expect(api.post).toHaveBeenCalledWith('/agents/transcripts/sync');
    expect(api.get).toHaveBeenCalledWith('/agents/transcripts/a%20b/context', { seq: 5 });
    expect(api.post).toHaveBeenCalledWith('/agents/transcripts/a%2Fb/resume', {});
  });
});
