import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { SourcesList } from './sources-list';

describe('SourcesList', () => {
  it('найденная папка — число разговоров, отсутствующая — «папки нет»', () => {
    const { container } = render(
      <SourcesList
        sources={[
          { provider: 'claude', root: '/c', exists: true, sessions: 7 },
          { provider: 'grok', root: '/g', exists: false, sessions: 0 },
        ]}
      />
    );
    expect(screen.getByText('7 разговоров')).toBeInTheDocument();
    expect(screen.getByText('папки нет')).toBeInTheDocument();
    expect(screen.getByText('/g')).toBeInTheDocument();
    expect(container.querySelectorAll('.sourceMissing')).toHaveLength(1);
  });
});
