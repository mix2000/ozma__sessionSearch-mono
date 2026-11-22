import { useEffect, useState } from 'react';

import { useDebouncedValue } from 'common/hooks';

import { SEARCH_DEBOUNCE_MS } from '../../transcripts.consts';
import type { ISearchController } from '../models';
import { buildSearch, parseSearchState } from '../utils/url-state';

/**
 * Состояние поиска страницы: текст в поле, запрос (с задержкой после ввода,
 * сразу — по Enter), фильтры. Начальное — из адреса, изменения — обратно
 * в адрес (replaceState: без лишних шагов «назад»).
 */
export function useSearchState(): ISearchController {
  const [initial] = useState(() => parseSearchState(window.location.search));
  const [input, setInput] = useState(initial.q);
  const [q, setQ] = useState(initial.q);
  const [provider, setProvider] = useState(initial.provider);
  const [includeHeadless, setIncludeHeadless] = useState(initial.includeHeadless);
  const debounced = useDebouncedValue(input, SEARCH_DEBOUNCE_MS);

  useEffect(() => {
    setQ(debounced.trim());
  }, [debounced]);

  useEffect(() => {
    const search = buildSearch(window.location.search, {
      q,
      provider,
      includeHeadless,
    });
    if (search !== window.location.search) {
      window.history.replaceState(
        null,
        '',
        window.location.pathname + search + window.location.hash
      );
    }
  }, [q, provider, includeHeadless]);

  function submit(value: string) {
    setInput(value);
    setQ(value.trim());
  }

  return {
    input,
    setInput,
    submit,
    q,
    provider,
    setProvider,
    includeHeadless,
    setIncludeHeadless,
  };
}
