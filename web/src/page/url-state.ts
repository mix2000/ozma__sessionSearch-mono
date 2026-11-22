// Запрос в адресной строке (?q=&provider=&headless=1): поиск можно
// сохранить в закладки или переслать внутри локальной утилиты.

export interface SearchState {
  q: string;
  provider: string;
  includeHeadless: boolean;
}

const KNOWN_PROVIDERS = ["claude", "codex", "grok"];

export function parseSearchState(search: string): SearchState {
  const params = new URLSearchParams(search);
  const provider = params.get("provider") ?? "";
  return {
    q: (params.get("q") ?? "").trim(),
    provider: KNOWN_PROVIDERS.includes(provider) ? provider : "",
    includeHeadless: params.get("headless") === "1",
  };
}

/** Новый search-часть URL: свои ключи заменяются, чужие остаются. */
export function buildSearch(current: string, state: SearchState): string {
  const params = new URLSearchParams(current);
  for (const key of ["q", "provider", "headless"]) params.delete(key);
  if (state.q) params.set("q", state.q);
  if (state.provider) params.set("provider", state.provider);
  if (state.includeHeadless) params.set("headless", "1");
  const s = params.toString();
  return s ? `?${s}` : "";
}
