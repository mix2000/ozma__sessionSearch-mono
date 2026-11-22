import { describe, expect, it } from 'vitest';

import { plural } from './plural';

const FORMS: [string, string, string] = ['символ', 'символа', 'символов'];

describe('plural', () => {
  it('1, 21 — первая форма', () => {
    expect(plural(1, FORMS)).toBe('символ');
    expect(plural(21, FORMS)).toBe('символ');
  });

  it('2–4, 22 — вторая форма', () => {
    expect(plural(2, FORMS)).toBe('символа');
    expect(plural(4, FORMS)).toBe('символа');
    expect(plural(22, FORMS)).toBe('символа');
  });

  it('0, 5–20, 11–14 — третья форма', () => {
    for (const n of [0, 5, 11, 12, 14, 20, 111]) expect(plural(n, FORMS)).toBe('символов');
  });
});
