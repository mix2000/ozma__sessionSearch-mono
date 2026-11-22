import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { makeHit } from 'common/test/test-utils';

import { HitSnippet } from './hit-snippet';

describe('HitSnippet', () => {
  it('абзац с подсветкой, соседние абзацы, «Контекст»', () => {
    const onContext = vi.fn();
    const { container } = render(
      <HitSnippet hit={makeHit()} provider="claude" terms={['nginx']} onContext={onContext} />
    );
    expect(screen.getByText('до')).toBeInTheDocument();
    expect(screen.getByText('после')).toBeInTheDocument();
    expect(container.querySelector('mark')).toHaveTextContent('nginx');
    fireEvent.click(screen.getByRole('button', { name: 'Контекст' }));
    expect(onContext).toHaveBeenCalled();
  });

  it('без соседних абзацев', () => {
    const { container } = render(
      <HitSnippet
        hit={makeHit({ snippet: { match: 'только' } })}
        provider="codex"
        terms={[]}
        onContext={() => {}}
      />
    );
    expect(container.querySelectorAll('pre')).toHaveLength(1);
  });
});
