import { afterEach, describe, expect, it, vi } from 'vitest';

import { folderName, formatRelative, formatShortDate } from './format';

describe('formatRelative', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('минуты, часы, вчера, дни', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-11-22T12:00:00Z'));
    expect(formatRelative('2026-11-22T11:59:30Z')).toBe('только что');
    expect(formatRelative('2026-11-22T11:55:00Z')).toBe('5 мин назад');
    expect(formatRelative('2026-11-22T09:00:00Z')).toBe('3 ч назад');
    expect(formatRelative('2026-11-21T10:00:00Z')).toBe('вчера');
    expect(formatRelative('2026-11-19T10:00:00Z')).toBe('3 дн назад');
    expect(formatRelative(null)).toBe('—');
  });
});

describe('formatShortDate', () => {
  it('пусто — прочерк', () => {
    expect(formatShortDate(null)).toBe('—');
    expect(formatShortDate('2026-11-22T12:00:00Z')).not.toBe('—');
  });
});

describe('folderName', () => {
  it('последний сегмент пути, домашняя — ~', () => {
    expect(folderName('/Users/me/work/repo/', '?')).toBe('repo');
    expect(folderName('/Users/me', '?')).toBe('~');
    expect(folderName('/home/me/', '?')).toBe('~');
    expect(folderName('C:\\Users\\me', '?')).toBe('~');
    expect(folderName('D:\\work\\repo', '?')).toBe('repo');
    expect(folderName(null, '?')).toBe('?');
  });
});
