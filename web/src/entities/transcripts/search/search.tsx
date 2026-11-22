import { Card, Space, Spin, Typography } from 'antd';
import { useState } from 'react';

import { PageHeader } from 'common/components';
import { apiErrorMessage } from 'common/services/api';

import { ContextModal } from '../context';

import { IndexStatus, ResultsSummary, SearchToolbar, TranscriptCard } from './components';
import { useSearchState, useTranscriptSearch, useTranscriptStatus } from './hooks';
import { SEARCH_TEXTS as T } from './search.consts';
import styles from './search.module.scss';

import type { IContextTarget } from '../context';

/**
 * Полнотекстовый поиск по разговорам агентов и продолжение.
 * Дети Space — по одному на элемент (antd оборачивает каждого в .ant-space-item),
 * поэтому условия и список карточек остаются здесь, а не в обёртке.
 */
export function TranscriptsSearch() {
  const state = useSearchState();
  const { q, provider, includeHeadless } = state;
  const minLength = useTranscriptStatus().data?.minQueryLength;
  const [context, setContext] = useState<IContextTarget | null>(null);
  const search = useTranscriptSearch({ q, provider, includeHeadless }, minLength);
  const terms = search.data?.terms ?? [];
  const results = search.data?.results ?? [];
  const long = minLength !== undefined && q.length >= minLength;

  return (
    <Space orientation="vertical" size="middle" className={styles.stack}>
      <PageHeader title={T.title} subtitle={T.subtitle} />
      <Card size="small">
        <SearchToolbar state={state} loading={search.isFetching} />
        <IndexStatus />
      </Card>
      {q.length > 0 && minLength !== undefined && !long && (
        <Typography.Text type="secondary">{T.minChars(minLength)}</Typography.Text>
      )}
      {search.isLoading && <Spin />}
      {search.isError && (
        <Typography.Text type="danger">
          {apiErrorMessage(search.error, T.searchFailed)}
        </Typography.Text>
      )}
      {search.data && long && (
        <>
          <ResultsSummary count={results.length} truncated={search.data.truncated} />
          {results.map(r => (
            <TranscriptCard
              key={r.id}
              result={r}
              terms={terms}
              onContext={seq => setContext({ result: r, seq })}
            />
          ))}
        </>
      )}
      <ContextModal target={context} terms={terms} onClose={() => setContext(null)} />
    </Space>
  );
}
