import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { makeStatus, testQueryClient } from 'common/test/test-utils';
import { QueryProvider } from 'providers';

import { App } from './app';

vi.mock('entities/transcripts/transcripts.api', () => ({
  transcriptsApi: { status: vi.fn().mockResolvedValue(makeStatus()), search: vi.fn() },
}));

describe('App', () => {
  it('тема, заголовок вкладки и страница поиска в контейнере', async () => {
    const { container } = render(
      <QueryProvider client={testQueryClient()}>
        <App />
      </QueryProvider>
    );
    expect(document.title).toBe('ozma Session Search');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(screen.getByRole('heading', { name: 'ozma Session Search' })).toBeInTheDocument();
    expect(container.querySelector('.page > .ant-space')).not.toBeNull();
    expect(await screen.findByText('В индексе 3 разговоров, 10 реплик')).toBeInTheDocument();
  });
});
