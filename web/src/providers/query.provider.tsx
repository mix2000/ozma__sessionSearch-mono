import { QueryClientProvider } from '@tanstack/react-query';

import { createQueryClient } from './query-client';

import type { QueryClient } from '@tanstack/react-query';
import type { ReactNode } from 'react';

const queryClient = createQueryClient();

interface IQueryProviderProps {
  children: ReactNode;
  /* Свой клиент (тесты). */
  client?: QueryClient;
}

export function QueryProvider({ children, client = queryClient }: IQueryProviderProps) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
