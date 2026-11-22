import { describe, expect, it } from 'vitest';

import { hiddenMatches, placeLabel } from './place';

describe('placeLabel', () => {
  it('проект / репозиторий из builder', () => {
    expect(placeLabel({ projectName: 'P', repoName: 'R', workdir: '/w/x' }, '?')).toBe('P / R');
  });

  it('иначе — имя папки или запасной текст', () => {
    expect(placeLabel({ projectName: 'P', workdir: '/w/x' }, '?')).toBe('x');
    expect(placeLabel({ workdir: null }, '?')).toBe('?');
  });
});

describe('hiddenMatches', () => {
  it('разница между совпадениями и показанными', () => {
    const hit = { messageId: 1, seq: 0, role: 'user', at: null, snippet: { match: '' } };
    expect(hiddenMatches({ matches: 5, hits: [hit, hit] })).toBe(3);
    expect(hiddenMatches({ matches: 1, hits: [hit] })).toBe(0);
  });
});
