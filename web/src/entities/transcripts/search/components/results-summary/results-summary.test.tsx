import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { ResultsSummary } from './results-summary';

describe('ResultsSummary', () => {
  it('число разговоров, пометка о свежих', () => {
    const { rerender } = render(<ResultsSummary count={2} truncated={false} />);
    expect(screen.getByText('Разговоров: 2')).toBeInTheDocument();
    rerender(<ResultsSummary count={40} truncated />);
    expect(screen.getByText('Разговоров: 40 (показаны самые свежие)')).toBeInTheDocument();
  });

  it('пусто — пустое состояние с подсказкой', () => {
    render(<ResultsSummary count={0} truncated={false} />);
    expect(screen.getByText('Ничего не найдено')).toBeInTheDocument();
    expect(screen.getByText(/Ищутся все слова в одной реплике/)).toBeInTheDocument();
  });
});
