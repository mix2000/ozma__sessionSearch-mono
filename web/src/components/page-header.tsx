import { Typography } from "antd";
import type { ReactNode } from "react";

import styles from "./page-header.module.scss";

interface Props {
  title: ReactNode;
  subtitle?: ReactNode;
  /** Правый блок: кнопки/тулбар. */
  extra?: ReactNode;
}

/** Шапка раздела, как в builder: заголовок + описание + правый toolbar. */
export function PageHeader({ title, subtitle, extra }: Props) {
  return (
    <div className={styles.header}>
      <div className={styles.text}>
        <Typography.Title level={4} className={styles.title}>
          {title}
        </Typography.Title>
        {subtitle && (
          <Typography.Text type="secondary" className={styles.subtitle}>
            {subtitle}
          </Typography.Text>
        )}
      </div>
      {extra && <div className={styles.extra}>{extra}</div>}
    </div>
  );
}
