import { formatRelative } from 'common/utils';

import type { ITranscriptStatus } from '../../models';
import type { IStatusLineTexts } from '../models';

/** Строка «В индексе N разговоров… · обновлено 5 мин назад». */
export function statusLine(status: ITranscriptStatus, texts: IStatusLineTexts): string {
  const index = status.index;
  const suffix = index?.running
    ? ` · ${texts.indexing}`
    : index?.lastRunAt
      ? ` · ${texts.updated(formatRelative(index.lastRunAt))}`
      : '';
  return texts.stats(status.sessions, status.messages) + suffix;
}
