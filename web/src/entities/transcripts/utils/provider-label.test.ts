import { describe, expect, it } from 'vitest';

import { providerLabel } from './provider-label';

describe('providerLabel', () => {
  it('известный агент — подпись', () => {
    expect(providerLabel('codex')).toBe('Codex');
  });

  it('неизвестный — как есть', () => {
    expect(providerLabel('gemini')).toBe('gemini');
  });
});
