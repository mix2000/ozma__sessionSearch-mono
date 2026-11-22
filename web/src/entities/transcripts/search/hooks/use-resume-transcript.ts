import { useMutation, useQueryClient } from '@tanstack/react-query';

import { queryKeys } from 'common/services/api';

import { transcriptsApi } from '../../transcripts.api';

/** Продолжить разговор: бэкенд открывает его в новом окне терминала. */
export function useResumeTranscript() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => transcriptsApi.resume(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.transcripts.all }),
  });
}
