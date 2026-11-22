/* Агент, чьи разговоры индексируются. */
export type TAgentKind = 'claude' | 'codex' | 'grok';

/* Разговор открыт в чьём-то терминале (утилита без tmux). */
export interface ILiveState {
  kind: 'external';
  pid: number;
}

/* Абзац с совпадением и соседние абзацы. */
export interface ISnippet {
  before?: string;
  match: string;
  after?: string;
}

/* Одно совпадение в разговоре. */
export interface ITranscriptHit {
  messageId: number;
  seq: number;
  role: string;
  at: string | null;
  snippet: ISnippet;
}

/* Найденный разговор. */
export interface ITranscriptResult {
  id: string;
  provider: TAgentKind;
  title: string | null;
  workdir: string | null;
  kind: string | null;
  gitBranch: string | null;
  startedAt: string | null;
  lastAt: string | null;
  messageCount: number;
  /* Поля builder: в утилите обычно нет, показываются, только если пришли. */
  projectName?: string;
  repoName?: string;
  repositoryId?: string;
  live?: ILiveState;
  workdirExists: boolean;
  matches: number;
  hits: ITranscriptHit[];
  /* Команда продолжения для своего терминала (собирает бэкенд). */
  resumeCommand?: string;
}

/* Ответ поиска. */
export interface ITranscriptSearchResponse {
  terms: string[];
  results: ITranscriptResult[];
  truncated: boolean;
}

/* Реплика разговора целиком (контекст вокруг совпадения). */
export interface ITranscriptMessage {
  id: number;
  seq: number;
  role: string;
  at: string | null;
  text: string;
}

/* Папка, которую индексирует утилита. */
export interface IProviderSource {
  provider: TAgentKind | string;
  root: string;
  exists: boolean;
  sessions: number;
}

/* Состояние фоновой индексации. */
export interface IIndexState {
  running: boolean;
  lastRunAt: string | null;
  lastDurationMs: number | null;
  lastError: string | null;
}

/* Состояние индекса. */
export interface ITranscriptStatus {
  sessions: number;
  messages: number;
  index: IIndexState;
  providers?: IProviderSource[];
  /* Минимальная длина запроса (символов), которую принимает сервер. */
  minQueryLength: number;
}

/* Итог ручного обновления индекса. */
export interface ISyncResponse {
  files: number;
  updated: number;
  messages: number;
}

/* Итог «Продолжить». */
export interface IResumeResponse {
  ok: boolean;
  command?: string;
}

/* Параметры запроса поиска. */
export interface ITranscriptSearchParams {
  q: string;
  provider?: string;
  includeHeadless?: boolean;
}

/* Агент в фильтре. */
export interface IProviderOption {
  value: TAgentKind;
  label: string;
}

/* Кусок текста для подсветки: совпадение с термином или обычный текст. */
export interface ITextPart {
  text: string;
  hit: boolean;
}

/* Цвета тегов агента: в шапке карточки и у его реплик. */
export interface IProviderColors {
  agent: string;
  reply: string;
}
