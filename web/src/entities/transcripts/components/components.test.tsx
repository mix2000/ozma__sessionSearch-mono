import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { HighlightedText } from './highlighted-text';
import { MessageHead } from './message-head';
import { MessageText } from './message-text';
import { RoleTag } from './role-tag';

describe('HighlightedText', () => {
  it('совпадения — в <mark>', () => {
    const { container } = render(<HighlightedText text="Почини NGINX и nginx" terms={['nginx']} />);
    const marks = container.querySelectorAll('mark');
    expect([...marks].map(m => m.textContent)).toEqual(['NGINX', 'nginx']);
  });
});

describe('MessageText', () => {
  it('с терминами — подсветка, без — как есть; dim приглушает', () => {
    const { container, rerender } = render(<MessageText text="a nginx" terms={['nginx']} />);
    expect(container.querySelector('pre mark')).toHaveTextContent('nginx');
    rerender(<MessageText text="соседний абзац" dim />);
    const pre = container.querySelector('pre')!;
    expect(pre).toHaveTextContent('соседний абзац');
    expect(pre.querySelector('mark')).toBeNull();
    expect(pre.className).toContain('dim');
  });
});

describe('RoleTag', () => {
  it('«Вы» для пользователя, название агента для ответа', () => {
    render(
      <>
        <RoleTag role="user" provider="codex" />
        <RoleTag role="assistant" provider="codex" />
        <RoleTag role="assistant" provider="other" />
      </>
    );
    expect(screen.getByText('Вы')).toBeInTheDocument();
    expect(screen.getByText('Codex')).toBeInTheDocument();
    expect(screen.getByText('other')).toBeInTheDocument();
  });
});

describe('MessageHead', () => {
  it('автор, дата и доп. действие; без даты — без даты', () => {
    const { rerender } = render(
      <MessageHead role="user" provider="claude" at="2026-11-10T08:55:00.000Z" extra={<b>ещё</b>} />
    );
    expect(screen.getByText('Вы')).toBeInTheDocument();
    expect(screen.getByText(/2026/)).toBeInTheDocument();
    expect(screen.getByText('ещё')).toBeInTheDocument();
    rerender(<MessageHead role="assistant" provider="claude" at={null} />);
    expect(screen.queryByText(/2026/)).toBeNull();
    expect(screen.getByText('Claude')).toBeInTheDocument();
  });
});
