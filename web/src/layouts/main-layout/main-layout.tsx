import styles from './main-layout.module.scss';

import type { ReactNode } from 'react';

interface IMainLayoutProps {
  children: ReactNode;
}

/** Контейнер страницы: ширина и отступы, как main-layout builder. */
export function MainLayout({ children }: IMainLayoutProps) {
  return <div className={styles.page}>{children}</div>;
}
