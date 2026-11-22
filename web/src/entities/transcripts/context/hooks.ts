import { useQuery } from '@tanstack/react-query';

import { queryKeys } from 'common/services/api';

import { transcriptsApi } from '../transcripts.api';

/** Реплики вокруг совпадения `seq`; без цели запрос не идёт. */
export function useTranscriptContext(sessionId: string, seq: number | null) {
  return useQuery({
    queryKey: queryKeys.transcripts.context(sessionId, seq ?? -1),
    queryFn: () => transcriptsApi.context(sessionId, seq),
    enabled: !!sessionId && seq !== null,
  });
}
