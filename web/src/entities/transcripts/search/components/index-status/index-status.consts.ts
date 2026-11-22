import { NETWORK_ERROR_MESSAGE } from 'common/services/api';

export const INDEX_STATUS_TEXTS = {
  stats: (sessions: number, messages: number) =>
    `В индексе ${sessions} разговоров, ${messages} реплик`,
  updated: (ago: string) => `обновлено ${ago}`,
  indexing: 'индексирую…',
  refresh: 'Обновить индекс',
  refreshed: (n: number) => (n ? `Обновлено разговоров: ${n}` : 'Новых реплик нет'),
  statusFailed: NETWORK_ERROR_MESSAGE,
  sources: 'Источники',
  sourcesTitle: 'Какие папки индексируются',
  searchHint: 'Поиск: / или Ctrl+K',
};
