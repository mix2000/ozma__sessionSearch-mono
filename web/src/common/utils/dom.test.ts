import { beforeAll, describe, expect, it, vi } from 'vitest';

import { isFocusShortcut, isTypingTarget } from './dom';

// Тесты идут в node: минимальная замена HTMLElement.
class FakeElement {
  isContentEditable = false;
  constructor(readonly tagName: string) {}
}

beforeAll(() => {
  vi.stubGlobal('HTMLElement', FakeElement);
});

const key = (k: string, mods: Partial<KeyboardEvent> = {}) => ({
  key: k,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  target: null,
  ...mods,
});

describe('isTypingTarget', () => {
  it('поля ввода и contenteditable', () => {
    expect(isTypingTarget(new FakeElement('INPUT') as never)).toBe(true);
    expect(isTypingTarget(new FakeElement('TEXTAREA') as never)).toBe(true);
    expect(isTypingTarget(new FakeElement('DIV') as never)).toBe(false);
    const editable = new FakeElement('DIV');
    editable.isContentEditable = true;
    expect(isTypingTarget(editable as never)).toBe(true);
    expect(isTypingTarget(null)).toBe(false);
  });
});

describe('isFocusShortcut', () => {
  it('/ и Cmd/Ctrl+K', () => {
    expect(isFocusShortcut(key('/'))).toBe(true);
    expect(isFocusShortcut(key('k', { metaKey: true }))).toBe(true);
    expect(isFocusShortcut(key('K', { ctrlKey: true }))).toBe(true);
  });

  it('остальное — нет', () => {
    expect(isFocusShortcut(key('k'))).toBe(false);
    expect(isFocusShortcut(key('k', { ctrlKey: true, altKey: true }))).toBe(false);
    expect(isFocusShortcut(key('/', { ctrlKey: true }))).toBe(false);
    const input = new FakeElement('INPUT') as unknown as EventTarget;
    expect(isFocusShortcut(key('/', { target: input }))).toBe(false);
    expect(isFocusShortcut(key('k', { metaKey: true, target: input }))).toBe(true);
  });
});
