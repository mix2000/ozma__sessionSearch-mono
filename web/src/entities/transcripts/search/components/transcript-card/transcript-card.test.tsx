import { fireEvent, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { makeHit, makeResult, renderWithProviders } from 'common/test/test-utils';

import { TranscriptCard } from './transcript-card';

vi.mock('entities/transcripts/transcripts.api', () => ({ transcriptsApi: {} }));

describe('TranscriptCard', () => {
  it('заголовок, агент, папка, ветка, совпадения, действия', () => {
    const onContext = vi.fn();
    renderWithProviders(
      <TranscriptCard
        result={makeResult({ matches: 5, hits: [makeHit({ seq: 9 })] })}
        terms={['nginx']}
        onContext={onContext}
      />
    );
    expect(screen.getByText('Починка nginx')).toBeInTheDocument();
    expect(screen.getAllByText('Claude').length).toBeGreaterThan(0);
    expect(screen.getByText('proj')).toBeInTheDocument();
    expect(screen.getByText('main')).toBeInTheDocument();
    expect(screen.getByText('/Users/demo/proj')).toBeInTheDocument();
    expect(screen.getByText('ещё совпадений: 4')).toBeInTheDocument();
    expect(screen.getByText(/12 реплик/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Контекст' }));
    expect(onContext).toHaveBeenCalledWith(9);
  });

  it('без названия, автоматический, открыт в терминале, папка неизвестна', () => {
    renderWithProviders(
      <TranscriptCard
        result={makeResult({
          title: null,
          kind: 'headless',
          live: { kind: 'external', pid: 42 },
          workdir: null,
          gitBranch: null,
          lastAt: null,
        })}
        terms={[]}
        onContext={() => {}}
      />
    );
    expect(screen.getByText('Без названия')).toBeInTheDocument();
    expect(screen.getByText('автоматический')).toBeInTheDocument();
    expect(screen.getByText('открыт в терминале (pid 42)')).toBeInTheDocument();
    expect(screen.getByText('папка неизвестна')).toBeInTheDocument();
    expect(screen.queryByText(/ещё совпадений/)).toBeNull();
  });
});
