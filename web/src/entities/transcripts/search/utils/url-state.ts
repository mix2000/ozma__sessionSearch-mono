// Запрос в адресной строке (?q=&provider=&headless=1): поиск можно
// сохранить в закладки или переслать внутри локальной утилиты.

import { KNOWN_PROVIDERS } from '../../transcripts.consts';
import type { ISearchState } from '../models';

export function parseSearchState(search: string): ISearchState {
  const params = new URLSearchParams(search);
  const provider = params.get('provider') ?? '';
  return {
    q: (params.get('q') ?? '').trim(),
    provider: KNOWN_PROVIDERS.includes(provider) ? provider : '',
    includeHeadless: params.get('headless') === '1',
  };
}

/** Новый search-часть URL: свои ключи заменяются, чужие остаются. */
export function buildSearch(current: string, state: ISearchState): string {
  const params = new URLSearchParams(current);
  for (const key of ['q', 'provider', 'headless']) params.delete(key);
  if (state.q) params.set('q', state.q);
  if (state.provider) params.set('provider', state.provider);
  if (state.includeHeadless) params.set('headless', '1');
  const s = params.toString();
  return s ? `?${s}` : '';
}
