import { describe, expect, it } from 'vitest';

import { makeStatus } from 'common/test/test-utils';

import type { IStatusLineTexts } from '../models';

import { statusLine } from './status-line';

const TEXTS: IStatusLineTexts = {
  stats: (s, m) => `${s}/${m}`,
  updated: ago => `upd ${ago}`,
  indexing: 'busy',
};
const base = makeStatus({
  sessions: 3,
  messages: 10,
  index: { running: false, lastRunAt: null, lastDurationMs: null, lastError: null },
});

describe('statusLine', () => {
  it('только счётчики, пока индекс не обновлялся', () => {
    expect(statusLine(base, TEXTS)).toBe('3/10');
  });

  it('идёт индексация', () => {
    expect(statusLine({ ...base, index: { ...base.index, running: true } }, TEXTS)).toBe(
      '3/10 · busy'
    );
  });

  it('время последнего обновления', () => {
    const lastRunAt = new Date().toISOString();
    expect(statusLine({ ...base, index: { ...base.index, lastRunAt } }, TEXTS)).toMatch(
      /^3\/10 · upd /
    );
  });
});
