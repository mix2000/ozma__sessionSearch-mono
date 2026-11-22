import { Card, Tag, Typography } from 'antd';

import { formatShortDate } from 'common/utils';

import type { ITranscriptResult } from '../../../models';
import { PROVIDER_COLORS } from '../../../transcripts.consts';
import { providerLabel } from '../../../utils/provider-label';
import { hiddenMatches, placeLabel } from '../../utils/place';
import { HitSnippet } from '../hit-snippet';
import { ResumeActions } from '../resume-actions';

import { LIVE_TAG_COLOR, TRANSCRIPT_CARD_TEXTS as T } from './transcript-card.consts';
import styles from './transcript-card.module.scss';

interface ITranscriptCardProps {
  result: ITranscriptResult;
  terms: string[];
  onContext: (seq: number) => void;
}

/** Карточка найденного разговора: заголовок, где шёл, совпадения, действия. */
export function TranscriptCard({ result, terms, onContext }: ITranscriptCardProps) {
  const hidden = hiddenMatches(result);
  return (
    <Card
      size="small"
      className={styles.card}
      title={
        <span className={styles.cardTitle}>
          {result.title || T.untitled}
          {result.live?.kind === 'external' && (
            <Tag color={LIVE_TAG_COLOR}>{T.liveExternal(result.live.pid)}</Tag>
          )}
        </span>
      }
    >
      <div className={styles.meta}>
        <Tag color={PROVIDER_COLORS[result.provider]?.agent ?? 'default'}>
          {providerLabel(result.provider)}
        </Tag>
        {result.kind === 'headless' && <Tag>{T.headlessTag}</Tag>}
        <Typography.Text strong>{placeLabel(result, T.workdirUnknown)}</Typography.Text>
        {result.gitBranch && <Tag>{result.gitBranch}</Tag>}
        <Typography.Text type="secondary">
          {result.lastAt ? formatShortDate(result.lastAt) : ''} · {T.messages(result.messageCount)}
        </Typography.Text>
      </div>
      {result.workdir && (
        <Typography.Text type="secondary" className={styles.path}>
          {result.workdir}
        </Typography.Text>
      )}
      {result.hits.map(hit => (
        <HitSnippet
          key={hit.messageId}
          hit={hit}
          provider={result.provider}
          terms={terms}
          onContext={() => onContext(hit.seq)}
        />
      ))}
      {hidden > 0 && <Typography.Text type="secondary">{T.more(hidden)}</Typography.Text>}
      <ResumeActions result={result} />
    </Card>
  );
}
