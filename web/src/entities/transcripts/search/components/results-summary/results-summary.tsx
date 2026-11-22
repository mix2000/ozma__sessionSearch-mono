import { Typography } from 'antd';

import { EmptyResults } from '../empty-results';

import { RESULTS_SUMMARY_TEXTS as T } from './results-summary.consts';

interface IResultsSummaryProps {
  count: number;
  truncated: boolean;
}

/** Над списком результатов: сколько найдено или пустое состояние. */
export function ResultsSummary({ count, truncated }: IResultsSummaryProps) {
  if (count === 0) return <EmptyResults />;
  return <Typography.Text type="secondary">{T.found(count, truncated)}</Typography.Text>;
}
