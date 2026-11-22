export type AgentKind = "claude" | "codex" | "grok";

/** Утилита без tmux: разговор может быть открыт только в чьём-то терминале. */
export type LiveState = { kind: "external"; pid: number };

export interface Snippet {
  before?: string;
  match: string;
  after?: string;
}

export interface TranscriptHit {
  messageId: number;
  seq: number;
  role: string;
  at: string | null;
  snippet: Snippet;
}

export interface TranscriptResult {
  id: string;
  provider: AgentKind;
  title: string | null;
  workdir: string | null;
  kind: string | null;
  gitBranch: string | null;
  startedAt: string | null;
  lastAt: string | null;
  messageCount: number;
  /** Поля builder: в утилите обычно нет, показываются, только если пришли. */
  projectName?: string;
  repoName?: string;
  repositoryId?: string;
  live?: LiveState;
  workdirExists: boolean;
  matches: number;
  hits: TranscriptHit[];
  /** Готовая команда продолжения от бэкенда (обязательна для grok). */
  resumeCommand?: string;
}

export interface TranscriptSearchResponse {
  terms: string[];
  results: TranscriptResult[];
  truncated: boolean;
}

export interface TranscriptMessage {
  id: number;
  seq: number;
  role: string;
  at: string | null;
  text: string;
}

export interface ProviderSource {
  provider: AgentKind | string;
  root: string;
  exists: boolean;
  sessions: number;
}

export interface TranscriptStatus {
  sessions: number;
  messages: number;
  index: {
    running: boolean;
    lastRunAt: string | null;
    lastDurationMs: number | null;
    lastError: string | null;
  };
  providers?: ProviderSource[];
}

export interface SyncResponse {
  files: number;
  updated: number;
  messages: number;
}

export interface ResumeResponse {
  ok: boolean;
  command?: string;
}
