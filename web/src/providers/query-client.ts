import { QueryClient } from '@tanstack/react-query';

/** Клиент react-query приложения: без опроса в фоне и при фокусе окна. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        refetchIntervalInBackground: false,
        refetchOnWindowFocus: false,
        staleTime: 30_000,
        retry: 1,
      },
    },
  });
}
