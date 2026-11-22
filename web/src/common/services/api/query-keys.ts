// Ключи react-query в одном месте: инвалидация по префиксу `all`.

export const queryKeys = {
  transcripts: {
    all: ['transcripts'] as const,
    search: (q: string, provider: string, headless: boolean) =>
      ['transcripts', 'search', q, provider, headless] as const,
    status: () => ['transcripts', 'status'] as const,
    context: (id: string, seq: number) => ['transcripts', 'context', id, seq] as const,
  },
};
