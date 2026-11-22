import { describe, expect, it } from 'vitest';

import { queryKeys } from './query-keys';

describe('queryKeys.transcripts', () => {
  it('все ключи начинаются с общего префикса (инвалидация по all)', () => {
    const t = queryKeys.transcripts;
    expect(t.search('q', 'codex', true)).toEqual(['transcripts', 'search', 'q', 'codex', true]);
    expect(t.status()).toEqual(['transcripts', 'status']);
    expect(t.context('id', 3)).toEqual(['transcripts', 'context', 'id', 3]);
    for (const key of [t.search('', '', false), t.status(), t.context('x', 0)]) {
      expect(key[0]).toBe(t.all[0]);
    }
  });
});
