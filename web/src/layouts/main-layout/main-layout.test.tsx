import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { MainLayout } from './main-layout';

describe('MainLayout', () => {
  it('оборачивает страницу в контейнер', () => {
    const { container } = render(
      <MainLayout>
        <p>содержимое</p>
      </MainLayout>
    );
    expect(container.firstElementChild?.className).toContain('page');
    expect(screen.getByText('содержимое')).toBeInTheDocument();
  });
});
