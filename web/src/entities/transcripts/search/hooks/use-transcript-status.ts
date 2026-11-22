import { useQuery } from '@tanstack/react-query';

import { queryKeys } from 'common/services/api';

import { transcriptsApi } from '../../transcripts.api';

/** Интервал опроса состояния индекса, мс. */
export const STATUS_REFETCH_MS = 30_000;

/** Состояние индекса, обновляется раз в 30 с. */
export function useTranscriptStatus() {
  return useQuery({
    queryKey: queryKeys.transcripts.status(),
    queryFn: () => transcriptsApi.status(),
    refetchInterval: STATUS_REFETCH_MS,
  });
}
