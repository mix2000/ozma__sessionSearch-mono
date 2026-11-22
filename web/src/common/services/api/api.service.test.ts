import { beforeEach, describe, expect, it, vi } from 'vitest';

import { apiService } from './api.service';

const request = vi.hoisted(() => vi.fn());
vi.mock('./api.instance', () => ({ request }));

describe('apiService', () => {
  beforeEach(() => {
    request.mockReset().mockResolvedValue('ok');
  });

  it('get — без параметров и с ними', async () => {
    await apiService.get('/a');
    await apiService.get('/b', { q: 'x' });
    expect(request).toHaveBeenNthCalledWith(1, 'GET', '/a', {});
    expect(request).toHaveBeenNthCalledWith(2, 'GET', '/b', { params: { q: 'x' } });
  });

  it('post — без тела и с телом', async () => {
    await expect(apiService.post('/s')).resolves.toBe('ok');
    await apiService.post('/r', {});
    expect(request).toHaveBeenNthCalledWith(1, 'POST', '/s', {});
    expect(request).toHaveBeenNthCalledWith(2, 'POST', '/r', { body: {} });
  });
});
