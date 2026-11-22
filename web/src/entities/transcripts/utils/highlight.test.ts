import { describe, expect, it } from 'vitest';

import { splitByTerms } from './highlight';

describe('splitByTerms', () => {
  it('подсвечивает термины без учёта регистра', () => {
    expect(splitByTerms('Деплой nginx и NGINX', ['nginx'])).toEqual([
      { text: 'Деплой ', hit: false },
      { text: 'nginx', hit: true },
      { text: ' и ', hit: false },
      { text: 'NGINX', hit: true },
    ]);
  });

  it('спецсимволы регэкспа в запросе безопасны', () => {
    expect(splitByTerms('a.b (x)', ['(x)'])).toEqual([
      { text: 'a.b ', hit: false },
      { text: '(x)', hit: true },
    ]);
  });

  it('без терминов — весь текст одним куском', () => {
    expect(splitByTerms('abc', ['', '  '])).toEqual([{ text: 'abc', hit: false }]);
  });

  it('не меняет переданный массив терминов', () => {
    const terms = ['a', 'abc'];
    splitByTerms('abc', terms);
    expect(terms).toEqual(['a', 'abc']);
  });
});
