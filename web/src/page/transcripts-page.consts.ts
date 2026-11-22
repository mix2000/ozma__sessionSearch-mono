import type { AgentKind } from "../api/models";

export const APP_NAME = "ozma Session Search";

export const PROVIDERS: { value: AgentKind; label: string }[] = [
  { value: "claude", label: "Claude" },
  { value: "codex", label: "Codex" },
  { value: "grok", label: "Grok" },
];

const PROVIDER_LABELS: Record<string, string> = Object.fromEntries(
  PROVIDERS.map((p) => [p.value, p.label]),
);

/** Цвет тега агента в карточке разговора. */
export const PROVIDER_TAG_COLOR: Record<string, string> = {
  claude: "purple",
  codex: "blue",
  grok: "cyan",
};

/** Цвет тега автора реплики (ответ агента). */
export const ROLE_TAG_COLOR: Record<string, string> = {
  claude: "purple",
  codex: "geekblue",
  grok: "cyan",
};

export function providerLabel(provider: string): string {
  return PROVIDER_LABELS[provider] ?? provider;
}

export const TRANSCRIPTS_TEXTS = {
  title: APP_NAME,
  subtitle:
    "Текст всех разговоров Claude, Codex и Grok на этом компьютере: ваши запросы и ответы агента. Найдите, где обсуждали, и продолжите разговор.",
  placeholder: "Что искать: слова или «фраза в кавычках»",
  searchHint: "Поиск: / или Ctrl+K",
  minChars: "Введите хотя бы 2 символа",
  allProviders: "Все агенты",
  headless: "Включая автоматические (claude -p, codex exec)",
  stats: (sessions: number, messages: number) =>
    `В индексе ${sessions} разговоров, ${messages} реплик`,
  updated: (ago: string) => `обновлено ${ago}`,
  indexing: "индексирую…",
  refresh: "Обновить индекс",
  refreshed: (n: number) =>
    n ? `Обновлено разговоров: ${n}` : "Новых реплик нет",
  statusFailed: "Сервер поиска не отвечает",
  sources: "Источники",
  sourcesTitle: "Какие папки индексируются",
  sourceMissing: "папки нет",
  sourceSessions: (n: number) => `${n} разговоров`,
  nothing: "Ничего не найдено",
  nothingHint:
    "Ищутся все слова в одной реплике. Попробуйте меньше слов или включите автоматические запуски.",
  found: (n: number, truncated: boolean) =>
    `Разговоров: ${n}${truncated ? " (показаны самые свежие)" : ""}`,
  untitled: "Без названия",
  you: "Вы",
  agent: providerLabel,
  more: (n: number) => `ещё совпадений: ${n}`,
  context: "Контекст",
  contextTitle: "Разговор вокруг совпадения",
  contextFailed: "Не удалось загрузить контекст",
  messages: (n: number) => `${n} реплик`,
  headlessTag: "автоматический",
  liveExternal: (pid: number) => `открыт в терминале (pid ${pid})`,
  alreadyOpen: "Уже открыт в терминале",
  resume: "Продолжить",
  resumed: "Открыто в терминале",
  resumeFailed: "Не удалось продолжить",
  noFolder: "Папки разговора уже нет — продолжить нельзя",
  copyCommand: "Команда для своего терминала",
  workdirUnknown: "папка неизвестна",
};
