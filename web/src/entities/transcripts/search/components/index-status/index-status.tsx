import { FolderOpenOutlined, ReloadOutlined } from '@ant-design/icons';
import { App as AntApp, Button, Popover, Typography } from 'antd';

import { apiErrorMessage } from 'common/services/api';

import { useSyncTranscripts, useTranscriptStatus } from '../../hooks';
import { statusLine } from '../../utils/status-line';
import { SourcesList } from '../sources-list';

import { INDEX_STATUS_TEXTS as T } from './index-status.consts';
import styles from './index-status.module.scss';

/** Состояние индекса: счётчики, ошибки, «Обновить индекс», источники. */
export function IndexStatus() {
  const { message } = AntApp.useApp();
  const status = useTranscriptStatus();
  const sync = useSyncTranscripts();
  const index = status.data?.index;
  const sources = status.data?.providers ?? [];

  async function handleSync() {
    try {
      const res = await sync.mutateAsync();
      message.success(T.refreshed(res.updated));
    } catch (err) {
      message.error(apiErrorMessage(err, T.refresh));
    }
  }

  return (
    <div className={styles.status}>
      {status.data && (
        <Typography.Text type="secondary">{statusLine(status.data, T)}</Typography.Text>
      )}
      {status.isError && (
        <Typography.Text type="danger">
          {apiErrorMessage(status.error, T.statusFailed)}
        </Typography.Text>
      )}
      {index?.lastError && <Typography.Text type="danger">{index.lastError}</Typography.Text>}
      <Button size="small" icon={<ReloadOutlined />} loading={sync.isPending} onClick={handleSync}>
        {T.refresh}
      </Button>
      {sources.length > 0 && (
        <Popover trigger="click" title={T.sourcesTitle} content={<SourcesList sources={sources} />}>
          <Button size="small" icon={<FolderOpenOutlined />}>
            {T.sources}
          </Button>
        </Popover>
      )}
      <Typography.Text type="secondary" className={styles.hint}>
        {T.searchHint}
      </Typography.Text>
    </div>
  );
}
