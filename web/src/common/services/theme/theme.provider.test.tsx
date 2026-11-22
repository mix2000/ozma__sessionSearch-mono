import { act, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { mockMatchMedia } from 'common/test/test-utils';

import { ThemeProvider } from './theme.provider';
import { useTheme } from './use-theme';

function Probe() {
  return <span>{useTheme().resolved}</span>;
}

describe('ThemeProvider', () => {
  it('следует системной теме на лету и ставит data-theme', () => {
    const media = mockMatchMedia();
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>
    );
    expect(screen.getByText('light')).toBeInTheDocument();
    expect(document.documentElement.dataset.theme).toBe('light');
    act(() => media.set('(prefers-color-scheme: dark)', true));
    expect(screen.getByText('dark')).toBeInTheDocument();
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('вне провайдера — светлая', () => {
    render(<Probe />);
    expect(screen.getByText('light')).toBeInTheDocument();
  });
});
