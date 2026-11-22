import { useQuery } from '@tanstack/react-query';

import { queryKeys } from 'common/services/api';

import { transcriptsApi } from '../../transcripts.api';

import type { ITranscriptSearchParams } from '../../models';

/**
 * Поиск; прошлый ответ держится до нового. Запрос не отправляется, пока порог
 * длины (`minQueryLength` из статуса сервера) неизвестен или не набран.
 */
export function useTranscriptSearch(params: ITranscriptSearchParams, minQueryLength?: number) {
  return useQuery({
    queryKey: queryKeys.transcripts.search(
      params.q,
      params.provider ?? '',
      !!params.includeHeadless
    ),
    queryFn: () => transcriptsApi.search(params),
    enabled: minQueryLength !== undefined && params.q.trim().length >= minQueryLength,
    placeholderData: prev => prev,
  });
}
