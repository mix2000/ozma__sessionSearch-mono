import { Empty, Typography } from 'antd';

import { EMPTY_RESULTS_TEXTS as T } from './empty-results.consts';

/** Пустой результат поиска с подсказкой. */
export function EmptyResults() {
  return (
    <Empty description={T.nothing}>
      <Typography.Text type="secondary">{T.nothingHint}</Typography.Text>
    </Empty>
  );
}
