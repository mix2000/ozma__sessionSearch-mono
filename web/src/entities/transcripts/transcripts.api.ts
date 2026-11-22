// Запросы сущности «разговоры» поверх общего API-сервиса.

import { apiService } from 'common/services/api';

import type {
  IResumeResponse,
  ISyncResponse,
  ITranscriptMessage,
  ITranscriptSearchParams,
  ITranscriptSearchResponse,
  ITranscriptStatus,
} from './models';

const BASE = '/agents/transcripts';

function sessionPath(id: string, action: string): string {
  return `${BASE}/${encodeURIComponent(id)}/${action}`;
}

export const transcriptsApi = {
  search(params: ITranscriptSearchParams) {
    return apiService.get<ITranscriptSearchResponse>(`${BASE}/search`, {
      q: params.q,
      provider: params.provider || undefined,
      includeHeadless: params.includeHeadless ? 'true' : undefined,
    });
  },
  status() {
    return apiService.get<ITranscriptStatus>(`${BASE}/status`);
  },
  sync() {
    return apiService.post<ISyncResponse>(`${BASE}/sync`);
  },
  context(sessionId: string, seq: number | null) {
    return apiService.get<ITranscriptMessage[]>(sessionPath(sessionId, 'context'), { seq });
  },
  /* Бэкенд открывает разговор в новом окне терминала. */
  resume(id: string) {
    return apiService.post<IResumeResponse>(sessionPath(id, 'resume'), {});
  },
};
