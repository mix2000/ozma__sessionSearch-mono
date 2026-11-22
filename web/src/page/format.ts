// Форматтеры из builder (common/utils/format.ts).

/** Короткая дата без секунд: «12.06.2026, 16:30». */
export function formatShortDate(iso: string | null | undefined): string {
  if (!iso) {
    return "—";
  }
  return new Date(iso).toLocaleString(undefined, {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Относительное время: «5 мин назад». */
export function formatRelative(iso: string | null | undefined): string {
  if (!iso) {
    return "—";
  }
  const diffMs = Date.now() - new Date(iso).getTime();
  const min = Math.floor(diffMs / 60000);
  if (min < 1) {
    return "только что";
  }
  if (min < 60) {
    return `${min} мин назад`;
  }
  const hours = Math.floor(min / 60);
  if (hours < 24) {
    return `${hours} ч назад`;
  }
  const days = Math.floor(hours / 24);
  return days === 1 ? "вчера" : `${days} дн назад`;
}

/** Короткое имя папки: последний сегмент пути, домашняя папка — «~». */
export function folderName(path: string | null, unknown: string): string {
  if (!path) return unknown;
  if (/^(\/Users\/[^/]+|\/home\/[^/]+|[A-Za-z]:\\Users\\[^\\]+)[/\\]?$/.test(path)) {
    return "~";
  }
  const parts = path.replace(/[/\\]+$/, "").split(/[/\\]/);
  return parts[parts.length - 1] || path;
}
