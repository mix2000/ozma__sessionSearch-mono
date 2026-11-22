import { Select as AntSelect } from 'antd';
import type { CSSProperties } from 'react';

import { useIsTouch } from 'common/hooks';
import type { ISelectOption } from 'common/types';

import styles from './select.module.scss';

interface ISimpleSelectProps {
  value: string;
  options: ISelectOption[];
  onChange: (value: string) => void;
  style?: CSSProperties;
  className?: string;
  'aria-label'?: string;
}

/**
 * Простой Select как в builder: на тач-устройствах — нативный <select>
 * (системный список iOS/Android), на десктопе — antd Select.
 */
export function SimpleSelect({
  value,
  options,
  onChange,
  style,
  className,
  ...rest
}: ISimpleSelectProps) {
  const touch = useIsTouch();
  if (!touch) {
    return (
      <AntSelect
        value={value}
        options={options}
        onChange={onChange}
        style={style}
        className={className}
        aria-label={rest['aria-label']}
      />
    );
  }
  return (
    <span className={`${styles.wrap} ${className ?? ''}`} style={style}>
      <select
        className={styles.select}
        value={value}
        aria-label={rest['aria-label']}
        onChange={e => onChange(e.target.value)}
      >
        {options.map(o => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </span>
  );
}
