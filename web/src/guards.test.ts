// @guard — сторожа правил фронта: обходят дерево src и падают на КЛАССЕ
// нарушения, а не на одном случае (как scripts/guards.sh в CheerPortal).
// Общий текст для ozma Session Search и ozma AI VPN Closer (scratchpad/shared-guards).
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, normalize, relative, sep } from 'node:path';

import { describe, expect, it } from 'vitest';

const SRC = __dirname;
/** Бюджет файла рабочего кода. */
const MAX_FILE_LINES = 500;
/** Бюджет тела компонента. */
const MAX_COMPONENT_LINES = 60;

interface ISource {
  path: string;
  text: string;
}

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap(name => {
    const full = join(dir, name);
    return statSync(full).isDirectory() ? walk(full) : [full];
  });
}

const posix = (p: string) => p.split(sep).join('/');
const all: ISource[] = walk(SRC).map(f => ({
  path: posix(relative(SRC, f)),
  text: readFileSync(f, 'utf8'),
}));
const isTest = (p: string) => /\.test\.tsx?$/.test(p);
/** Рабочий код: .ts/.tsx/.scss без тестов. */
const code = all.filter(f => /\.(ts|tsx|scss)$/.test(f.path) && !isTest(f.path));
const scripts = code.filter(f => /\.tsx?$/.test(f.path));
/** Мок, тестовые обёртки и настройка тестов — инструменты тестов, не продукт. */
const tooling = (p: string) =>
  p.startsWith('mocks/') ||
  p.startsWith('mock/') ||
  p.startsWith('common/test/') ||
  p === 'test-setup.ts';

/** Длина файла как у `wc -l`: перевод строки в конце не добавляет строку. */
export function lineCount(text: string): number {
  const parts = text.split('\n');
  return parts[parts.length - 1] === '' ? parts.length - 1 : parts.length;
}

/** Код без комментариев (строки сохраняют номера). */
export function stripComments(text: string): string {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, m => m.replace(/[^\n]/g, ' '))
    .replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');
}

/** Импортируемые модули файла. */
export function importsOf(text: string): string[] {
  const out: string[] = [];
  for (const m of text.matchAll(/(?:import|export)[^'"`;]*?from\s+['"]([^'"]+)['"]/g))
    out.push(m[1]);
  for (const m of text.matchAll(/import\(\s*['"]([^'"]+)['"]\s*\)/g)) out.push(m[1]);
  return out;
}

/** Куда ведёт импорт (относительно src), если это наш модуль. */
export function resolveImport(from: string, spec: string): string | null {
  if (spec.startsWith('.')) return posix(normalize(join(dirname(from), spec)));
  if (/^(common|entities|pages|layouts|providers|mocks|mock|features|api)(\/|$)/.test(spec))
    return spec;
  return null;
}

/** Сущность пути `entities/<x>/…`. */
const entityOf = (p: string) => /^entities\/([^/]+)/.exec(p)?.[1] ?? null;

/** Компоненты файла: имя и длина тела в строках. */
export function components(text: string): { name: string; lines: number }[] {
  const lines = text.split('\n');
  const out: { name: string; lines: number }[] = [];
  lines.forEach((line, i) => {
    const m = /^export function ([A-Z]\w*)\s*[(<]/.exec(line);
    if (!m) return;
    let depth = 0;
    let opened = false;
    for (let k = i; k < lines.length; k += 1) {
      for (const ch of lines[k]) {
        if (ch === '{') {
          depth += 1;
          opened = true;
        } else if (ch === '}') depth -= 1;
      }
      // Деструктуризация параметров открывается и закрывается раньше тела,
      // поэтому конец функции — строка, после которой счётчик вернулся к нулю.
      if (opened && depth === 0) {
        out.push({ name: m[1], lines: k - i + 1 });
        return;
      }
    }
  });
  return out;
}

const pascal = (file: string) =>
  file
    .replace(/\.tsx$/, '')
    .split(/[-.]/)
    .map(w => w.charAt(0).toUpperCase() + w.slice(1))
    .join('');

describe('@guard сторожа фронта', () => {
  it('самопроверка: обход нашёл дерево', () => {
    expect(scripts.length).toBeGreaterThan(30);
    expect(scripts.some(f => f.path.startsWith('entities/'))).toBe(true);
    expect(scripts.some(f => f.path.startsWith('common/services/api/'))).toBe(true);
  });

  it(`файл рабочего кода не длиннее ${MAX_FILE_LINES} строк`, () => {
    const long = code
      .filter(f => lineCount(f.text) > MAX_FILE_LINES)
      .map(f => `${f.path} → ${lineCount(f.text)} строк`);
    expect(long).toEqual([]);
  });

  it('явные реэкспорты, без export * (в любом файле)', () => {
    const star = scripts.filter(f => /^\s*export \*/m.test(stripComments(f.text))).map(f => f.path);
    expect(star).toEqual([]);
  });

  it('к транспорту — только через api-сервис', () => {
    const bad = scripts
      .filter(
        f => !f.path.startsWith('common/services/api/') && !tooling(f.path) && f.path !== 'main.tsx'
      )
      .flatMap(f =>
        importsOf(f.text)
          .filter(s => /@tauri-apps\/api|api\.instance|^(mocks|mock)(\/|$)/.test(s))
          .map(s => `${f.path} → ${s}`)
      );
    const api = all.find(f => f.path === 'common/services/api/index.ts');
    if (api && /\b(call|listen|request)\b/.test(api.text.replace(/\/\/.*$/gm, '')))
      bad.push('common/services/api/index.ts реэкспортирует call/listen/request');
    expect(bad).toEqual([]);
  });

  it('кириллица в коде — только в *.consts.ts (тесты, моки и комментарии не в счёт)', () => {
    const bad = scripts
      .filter(f => !/\.consts\.ts$/.test(f.path) && !tooling(f.path))
      .flatMap(f =>
        stripComments(f.text)
          .split('\n')
          .map((l, i) => [l, i] as const)
          .filter(([l]) => /[А-Яа-яЁё]/.test(l))
          .map(([l, i]) => `${f.path}:${i + 1} ${l.trim()}`)
      );
    expect(bad).toEqual([]);
  });

  it('между сущностями и со страниц — только через index.ts сущности', () => {
    /* Тестовые обёртки (tooling) собирают фикстуры из моделей — им можно. */
    const bad = scripts
      .filter(f => !tooling(f.path))
      .flatMap(f =>
        importsOf(f.text)
          .map(s => [s, resolveImport(f.path, s)] as const)
          .filter(([, to]) => {
            const target = to && entityOf(to);
            if (!to || !target || target === entityOf(f.path)) return false;
            return to !== `entities/${target}` && to !== `entities/${target}/index`;
          })
          .map(([s]) => `${f.path} → ${s}`)
      );
    expect(bad).toEqual([]);
  });

  it('нет inline style={{…}} (исключение — `style-ok:` в комментарии рядом)', () => {
    const bad = scripts
      .filter(f => f.path.endsWith('.tsx'))
      .flatMap(f => {
        const lines = f.text.split('\n');
        return lines
          .map((l, i) => [l, i] as const)
          .filter(
            ([l, i]) => l.includes('style={{') && !`${lines[i - 1] ?? ''}${l}`.includes('style-ok:')
          )
          .map(([l, i]) => `${f.path}:${i + 1} ${l.trim()}`);
      });
    expect(bad).toEqual([]);
  });

  it(`тело компонента не длиннее ${MAX_COMPONENT_LINES} строк`, () => {
    const bad = scripts
      .filter(f => f.path.endsWith('.tsx') && !tooling(f.path))
      .flatMap(f =>
        components(f.text)
          .filter(c => c.lines > MAX_COMPONENT_LINES)
          .map(c => `${f.path} ${c.name} → ${c.lines} строк`)
      );
    expect(bad).toEqual([]);
  });

  /* Имя компонента оканчивается именем файла: list.tsx → ContestsList, search.tsx → TranscriptsSearch. */
  it('имя файла = имя компонента (kebab-case ↔ PascalCase)', () => {
    const bad = scripts
      .filter(f => f.path.endsWith('.tsx') && !tooling(f.path))
      .flatMap(f => {
        const file = f.path.split('/').pop() as string;
        return components(f.text)
          .filter(c => !c.name.endsWith(pascal(file)))
          .map(c => `${f.path}: ${c.name} ≠ ${pascal(file)}`);
      });
    expect(bad).toEqual([]);
  });

  it('детекторы сторожей работают (образцы)', () => {
    expect(stripComments('// Привет\nconst a = 1; /* мир */')).not.toMatch(/[А-Яа-я]/);
    expect(stripComments("const u = 'https://x';")).toContain('https://x');
    expect(
      importsOf("import { a } from 'entities/vpn/watch/models';\nconst x = import('mocks');")
    ).toEqual(['entities/vpn/watch/models', 'mocks']);
    expect(resolveImport('entities/monitor/status/a.ts', '../../vpn/watch/x')).toBe(
      'entities/vpn/watch/x'
    );
    expect(resolveImport('pages/p.tsx', 'react')).toBeNull();
    const long = `export function Big() {\n${'  x();\n'.repeat(MAX_COMPONENT_LINES)}}\n`;
    expect(components(long)).toEqual([{ name: 'Big', lines: MAX_COMPONENT_LINES + 2 }]);
    expect(pascal('theme.provider.tsx')).toBe('ThemeProvider');
    expect('TranscriptsSearch'.endsWith(pascal('search.tsx'))).toBe(true);
    expect(lineCount('a\nb\n')).toBe(2);
    expect(lineCount('a\nb')).toBe(2);
    expect('SearchBar'.endsWith(pascal('search.tsx'))).toBe(false);
  });
});
