import { useQueryClient } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { createQueryClient } from './query-client';
import { QueryProvider } from './query.provider';

describe('QueryProvider', () => {
  it('клиент по умолчанию: без опроса в фоне и при фокусе, один повтор', () => {
    const q = createQueryClient().getDefaultOptions().queries!;
    expect(q).toMatchObject({
      refetchIntervalInBackground: false,
      refetchOnWindowFocus: false,
      staleTime: 30_000,
      retry: 1,
    });
  });

  it('даёт клиент детям', () => {
    let seen: unknown = null;
    function Probe() {
      seen = useQueryClient();
      return null;
    }
    render(
      <QueryProvider>
        <Probe />
      </QueryProvider>
    );
    expect(seen).not.toBeNull();
  });
});
