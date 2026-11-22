import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { NETWORK_ERROR_MESSAGE } from './api.consts';
import { ApiError } from './api.errors';
import { captureTokenFromUrl, request, resetMemoryToken } from './api.instance';

function jsonResponse(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

describe('request', () => {
  const fetchMock = vi.fn();

  beforeEach(() => {
    vi.stubGlobal('fetch', fetchMock);
    fetchMock.mockReset();
    resetMemoryToken();
    sessionStorage.clear();
    window.history.replaceState(null, '', '/');
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('GET с параметрами: пустые и undefined не уходят', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ ok: 1 }));
    const res = await request<{ ok: number }>('GET', '/x', {
      params: { q: 'nginx', provider: '', n: 0, none: undefined, nil: null },
    });
    expect(res).toEqual({ ok: 1 });
    const [url, init] = fetchMock.mock.calls[0];
    expect(String(url)).toBe(`${window.location.origin}/api/x?q=nginx&n=0`);
    expect(init.method).toBe('GET');
    expect(init.headers).toEqual({ Accept: 'application/json' });
    expect(init.body).toBeUndefined();
  });

  it('POST с телом: JSON и Content-Type', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ ok: true }));
    await request('POST', '/r', { body: {} });
    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers['Content-Type']).toBe('application/json');
    expect(init.body).toBe('{}');
  });

  it('204 — undefined', async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }));
    await expect(request('POST', '/r')).resolves.toBeUndefined();
  });

  it('ошибка сервера — ApiError со статусом и сообщением', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ statusCode: 409, message: 'занят' }, 409));
    const err = await request('POST', '/r').catch(e => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ status: 409, message: 'занят' });
  });

  it('сеть недоступна — ApiError 0', async () => {
    fetchMock.mockRejectedValue(new TypeError('Failed to fetch'));
    await expect(request('GET', '/x')).rejects.toMatchObject({
      status: 0,
      message: NETWORK_ERROR_MESSAGE,
    });
  });

  it('?token= забирается из адреса и уходит в Authorization', async () => {
    window.history.replaceState(null, '', '/?q=a&token=s3cr%2Bt#h');
    captureTokenFromUrl();
    expect(window.location.search).toBe('?q=a');
    expect(window.location.hash).toBe('#h');
    expect(sessionStorage.getItem('session-search-token')).toBe('s3cr+t');
    fetchMock.mockResolvedValue(jsonResponse({}));
    await request('GET', '/x');
    expect(fetchMock.mock.calls[0][1].headers.Authorization).toBe('Bearer s3cr+t');
  });

  it('без токена в адресе ничего не меняется; токен из sessionStorage', async () => {
    window.history.replaceState(null, '', '/?q=a');
    captureTokenFromUrl();
    expect(window.location.search).toBe('?q=a');
    sessionStorage.setItem('session-search-token', 'stored');
    fetchMock.mockResolvedValue(jsonResponse({}));
    await request('GET', '/x');
    expect(fetchMock.mock.calls[0][1].headers.Authorization).toBe('Bearer stored');
  });
});
