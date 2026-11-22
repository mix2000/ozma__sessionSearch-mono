import { useMutation, useQueryClient } from '@tanstack/react-query';

import { queryKeys } from 'common/services/api';

import { transcriptsApi } from '../../transcripts.api';

/** «Обновить индекс»: после успеха перечитываются поиск и статус. */
export function useSyncTranscripts() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => transcriptsApi.sync(),
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.transcripts.all }),
  });
}
