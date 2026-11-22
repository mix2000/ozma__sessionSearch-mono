import { Select as AntSelect } from "antd";
import type { CSSProperties } from "react";
import { useSyncExternalStore } from "react";

import styles from "./select.module.scss";

const TOUCH_QUERY = "(pointer: coarse)";

function subscribe(onChange: () => void) {
  const mql = window.matchMedia?.(TOUCH_QUERY);
  mql?.addEventListener("change", onChange);
  return () => mql?.removeEventListener("change", onChange);
}

function isTouch() {
  return window.matchMedia?.(TOUCH_QUERY).matches ?? false;
}

interface Option {
  value: string;
  label: string;
}

interface Props {
  value: string;
  options: Option[];
  onChange: (value: string) => void;
  style?: CSSProperties;
  className?: string;
  "aria-label"?: string;
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
}: Props) {
  const touch = useSyncExternalStore(subscribe, isTouch, () => false);
  if (!touch) {
    return (
      <AntSelect
        value={value}
        options={options}
        onChange={onChange}
        style={style}
        className={className}
        aria-label={rest["aria-label"]}
      />
    );
  }
  return (
    <span className={`${styles.wrap} ${className ?? ""}`} style={style}>
      <select
        className={styles.select}
        value={value}
        aria-label={rest["aria-label"]}
        onChange={(e) => onChange(e.target.value)}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </span>
  );
}
