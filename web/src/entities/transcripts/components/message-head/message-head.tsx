import { Typography } from 'antd';
import type { ReactNode } from 'react';

import { formatShortDate } from 'common/utils';

import { RoleTag } from '../role-tag';

import styles from './message-head.module.scss';

interface IMessageHeadProps {
  role: string;
  provider: string;
  at: string | null;
  /** Справа от даты: например, кнопка «Контекст». */
  extra?: ReactNode;
}

/** Шапка реплики: автор, время, доп. действие. */
export function MessageHead({ role, provider, at, extra }: IMessageHeadProps) {
  return (
    <div className={styles.head}>
      <RoleTag role={role} provider={provider} />
      {at && <Typography.Text type="secondary">{formatShortDate(at)}</Typography.Text>}
      {extra}
    </div>
  );
}
