// Общие помощники компонентных тестов: свежий react-query без повторов,
// antd App (для message) и фабрики данных.
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, renderHook } from '@testing-library/react';
import { App as AntApp } from 'antd';

import type {
  ITranscriptHit,
  ITranscriptResult,
  ITranscriptStatus,
} from 'entities/transcripts/models';
import type { ReactElement, ReactNode } from 'react';

export function testQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: Infinity }, mutations: { retry: false } },
  });
}

function wrapperFor(client: QueryClient) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>
        <AntApp>{children}</AntApp>
      </QueryClientProvider>
    );
  };
}

export function renderWithProviders(ui: ReactElement, client = testQueryClient()) {
  return { client, ...render(ui, { wrapper: wrapperFor(client) }) };
}

export function renderHookWithProviders<T>(hook: () => T, client = testQueryClient()) {
  return { client, ...renderHook(hook, { wrapper: wrapperFor(client) }) };
}

/** matchMedia, у которого совпадают заданные запросы; возвращает «переключатель». */
export function mockMatchMedia(matching: string[] = []) {
  const listeners = new Map<string, Set<() => void>>();
  const state = new Set(matching);
  window.matchMedia = (query: string) =>
    ({
      get matches() {
        return state.has(query);
      },
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: (_: string, cb: () => void) => {
        if (!listeners.has(query)) listeners.set(query, new Set());
        listeners.get(query)!.add(cb);
      },
      removeEventListener: (_: string, cb: () => void) => listeners.get(query)?.delete(cb),
      dispatchEvent: () => false,
    }) as unknown as MediaQueryList;
  return {
    set(query: string, matches: boolean) {
      if (matches) state.add(query);
      else state.delete(query);
      listeners.get(query)?.forEach(cb => cb());
    },
    listenerCount: (query: string) => listeners.get(query)?.size ?? 0,
  };
}

export function makeHit(over: Partial<ITranscriptHit> = {}): ITranscriptHit {
  return {
    messageId: 1,
    seq: 4,
    role: 'user',
    at: '2026-11-10T08:55:00.000Z',
    snippet: { before: 'до', match: 'почини nginx', after: 'после' },
    ...over,
  };
}

export function makeResult(over: Partial<ITranscriptResult> = {}): ITranscriptResult {
  return {
    id: 'aaaa-1111',
    provider: 'claude',
    title: 'Починка nginx',
    workdir: '/Users/demo/proj',
    kind: 'interactive',
    gitBranch: 'main',
    startedAt: '2026-11-10T08:00:00.000Z',
    lastAt: '2026-11-10T09:00:00.000Z',
    messageCount: 12,
    workdirExists: true,
    matches: 1,
    hits: [makeHit()],
    ...over,
  };
}

export function makeStatus(over: Partial<ITranscriptStatus> = {}): ITranscriptStatus {
  return {
    sessions: 3,
    messages: 10,
    index: { running: false, lastRunAt: null, lastDurationMs: null, lastError: null },
    providers: [{ provider: 'claude', root: '/c', exists: true, sessions: 3 }],
    minQueryLength: 2,
    ...over,
  };
}
