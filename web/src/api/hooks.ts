import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { request } from "./client";
import type {
  ResumeResponse,
  SyncResponse,
  TranscriptMessage,
  TranscriptSearchResponse,
  TranscriptStatus,
} from "./models";

export const queryKeys = {
  all: ["transcripts"] as const,
  search: (q: string, provider: string, headless: boolean) =>
    ["transcripts", "search", q, provider, headless] as const,
  status: () => ["transcripts", "status"] as const,
  context: (id: string, seq: number) =>
    ["transcripts", "context", id, seq] as const,
};

export interface TranscriptSearchParams {
  q: string;
  provider?: string;
  includeHeadless?: boolean;
}

export function useTranscriptSearch(params: TranscriptSearchParams) {
  function search() {
    return request<TranscriptSearchResponse>("GET", "/agents/transcripts/search", {
      params: {
        q: params.q,
        provider: params.provider || undefined,
        includeHeadless: params.includeHeadless ? "true" : undefined,
      },
    });
  }
  return useQuery({
    queryKey: queryKeys.search(
      params.q,
      params.provider ?? "",
      !!params.includeHeadless,
    ),
    queryFn: search,
    enabled: params.q.trim().length >= 2,
    placeholderData: (prev) => prev,
  });
}

export function useTranscriptStatus() {
  function fetchStatus() {
    return request<TranscriptStatus>("GET", "/agents/transcripts/status");
  }
  return useQuery({
    queryKey: queryKeys.status(),
    queryFn: fetchStatus,
    refetchInterval: 30_000,
  });
}

export function useSyncTranscripts() {
  const qc = useQueryClient();
  function sync() {
    return request<SyncResponse>("POST", "/agents/transcripts/sync");
  }
  return useMutation({
    mutationFn: sync,
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.all }),
  });
}

export function useTranscriptContext(sessionId: string, seq: number | null) {
  function fetchContext() {
    return request<TranscriptMessage[]>(
      "GET",
      `/agents/transcripts/${encodeURIComponent(sessionId)}/context`,
      { params: { seq } },
    );
  }
  return useQuery({
    queryKey: queryKeys.context(sessionId, seq ?? -1),
    queryFn: fetchContext,
    enabled: !!sessionId && seq !== null,
  });
}

/** Продолжить разговор: бэкенд открывает его в новом окне терминала. */
export function useResumeTranscript() {
  const qc = useQueryClient();
  function resume(id: string) {
    return request<ResumeResponse>(
      "POST",
      `/agents/transcripts/${encodeURIComponent(id)}/resume`,
      { body: {} },
    );
  }
  return useMutation({
    mutationFn: resume,
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.all }),
  });
}
