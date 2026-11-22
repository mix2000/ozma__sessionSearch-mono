import { Tag, Typography } from 'antd';

import type { IProviderSource } from '../../../models';
import { PROVIDER_COLORS } from '../../../transcripts.consts';
import { providerLabel } from '../../../utils/provider-label';

import { SOURCES_LIST_TEXTS as T } from './sources-list.consts';
import styles from './sources-list.module.scss';

interface ISourcesListProps {
  sources: IProviderSource[];
}

/** Какие папки индексируются и сколько в них разговоров. */
export function SourcesList({ sources }: ISourcesListProps) {
  return (
    <div className={styles.sources}>
      {sources.map(s => (
        <div
          key={`${s.provider}:${s.root}`}
          className={`${styles.source} ${s.exists ? '' : styles.sourceMissing}`}
        >
          <div className={styles.sourceHead}>
            <Tag color={s.exists ? PROVIDER_COLORS[s.provider]?.agent : undefined}>
              {providerLabel(s.provider)}
            </Tag>
            <Typography.Text type="secondary">
              {s.exists ? T.sourceSessions(s.sessions) : T.sourceMissing}
            </Typography.Text>
          </div>
          <Typography.Text code className={styles.path}>
            {s.root}
          </Typography.Text>
        </div>
      ))}
    </div>
  );
}
