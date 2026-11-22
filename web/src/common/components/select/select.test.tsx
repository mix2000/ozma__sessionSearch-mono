import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { mockMatchMedia } from 'common/test/test-utils';

import { SimpleSelect } from './select';

const options = [
  { value: '', label: 'Все агенты' },
  { value: 'codex', label: 'Codex' },
];

describe('SimpleSelect', () => {
  it('на десктопе — antd Select', () => {
    mockMatchMedia();
    const { container } = render(
      <SimpleSelect value="" options={options} onChange={() => {}} aria-label="Агент" />
    );
    expect(container.querySelector('.ant-select')).not.toBeNull();
    expect(screen.queryByRole('option')).toBeNull();
  });

  it('на тач-устройстве — нативный select', () => {
    mockMatchMedia(['(pointer: coarse)']);
    const onChange = vi.fn();
    render(
      <SimpleSelect
        value=""
        options={options}
        onChange={onChange}
        aria-label="Агент"
        className="x"
      />
    );
    const select = screen.getByRole('combobox', { name: 'Агент' });
    expect(screen.getAllByRole('option')).toHaveLength(2);
    fireEvent.change(select, { target: { value: 'codex' } });
    expect(onChange).toHaveBeenCalledWith('codex');
  });
});
