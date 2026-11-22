/** Разбить текст на куски: совпадения с терминами (без учёта регистра) и остальное. */
export function splitByTerms(
  text: string,
  terms: string[],
): { text: string; hit: boolean }[] {
  const clean = terms.filter((t) => t.trim().length > 0);
  if (!clean.length) return [{ text, hit: false }];
  const escaped = [...clean]
    .sort((a, b) => b.length - a.length)
    .map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const re = new RegExp(`(${escaped.join("|")})`, "gi");
  const parts: { text: string; hit: boolean }[] = [];
  let last = 0;
  for (const m of text.matchAll(re)) {
    const idx = m.index ?? 0;
    if (idx > last) parts.push({ text: text.slice(last, idx), hit: false });
    parts.push({ text: m[0], hit: true });
    last = idx + m[0].length;
  }
  if (last < text.length) parts.push({ text: text.slice(last), hit: false });
  return parts;
}

/**
 * Папка в кавычках для `cd`. Двойные кавычки понимают и sh/zsh, и cmd,
 * и PowerShell, поэтому Windows-путь с `\` остаётся как есть. Если в
 * POSIX-пути есть символы, которые раскрываются внутри двойных кавычек
 * (`"`, `$`, `` ` ``), — одинарные кавычки, как в builder.
 */
export function quoteDir(dir: string): string {
  if (/["$`]/.test(dir) && !dir.includes("\\")) {
    return `'${dir.replace(/'/g, "'\\''")}'`;
  }
  return `"${dir}"`;
}

/**
 * Команда, чтобы продолжить разговор в своём терминале. Для claude/codex
 * собирается здесь; для grok формат знает только бэкенд — null.
 */
export function resumeCommand(
  provider: string,
  id: string,
  workdir: string | null,
): string | null {
  const dir = workdir ? `cd ${quoteDir(workdir)} && ` : "";
  if (provider === "codex") return `${dir}codex resume ${id}`;
  if (provider === "claude") return `${dir}claude --resume ${id}`;
  return null;
}
