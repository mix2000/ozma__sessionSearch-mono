import { folderName } from 'common/utils';

import type { ITranscriptResult } from '../../models';

/**
 * Где шёл разговор: «проект / репозиторий» (поля builder, если пришли)
 * или короткое имя рабочей папки.
 */
export function placeLabel(
  result: Pick<ITranscriptResult, 'projectName' | 'repoName' | 'workdir'>,
  unknown: string
): string {
  if (result.projectName && result.repoName) {
    return `${result.projectName} / ${result.repoName}`;
  }
  return folderName(result.workdir, unknown);
}

/** Сколько совпадений не показано в карточке (0 — все показаны). */
export function hiddenMatches(result: Pick<ITranscriptResult, 'matches' | 'hits'>): number {
  return Math.max(0, result.matches - result.hits.length);
}
