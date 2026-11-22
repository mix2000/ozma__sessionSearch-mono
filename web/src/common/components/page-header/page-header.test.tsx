import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { PageHeader } from './page-header';

describe('PageHeader', () => {
  it('заголовок, подзаголовок и правый блок', () => {
    render(<PageHeader title="Поиск" subtitle="описание" extra={<button>Кнопка</button>} />);
    expect(screen.getByRole('heading', { level: 4, name: 'Поиск' })).toBeInTheDocument();
    expect(screen.getByText('описание')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Кнопка' })).toBeInTheDocument();
  });

  it('без подзаголовка и правого блока', () => {
    const { container } = render(<PageHeader title="Только" />);
    expect(container.querySelectorAll('.ant-typography')).toHaveLength(1);
  });
});
