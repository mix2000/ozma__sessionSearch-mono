import type { ITextPart } from '../models';

/** Разбить текст на куски: совпадения с терминами (без учёта регистра) и остальное. */
export function splitByTerms(text: string, terms: string[]): ITextPart[] {
  const clean = terms.filter(t => t.trim().length > 0);
  if (!clean.length) return [{ text, hit: false }];
  const escaped = [...clean]
    .sort((a, b) => b.length - a.length)
    .map(t => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  const re = new RegExp(`(${escaped.join('|')})`, 'gi');
  const parts: ITextPart[] = [];
  let last = 0;
  for (const m of text.matchAll(re)) {
    const idx = m.index;
    if (idx > last) parts.push({ text: text.slice(last, idx), hit: false });
    parts.push({ text: m[0], hit: true });
    last = idx + m[0].length;
  }
  if (last < text.length) parts.push({ text: text.slice(last), hit: false });
  return parts;
}
