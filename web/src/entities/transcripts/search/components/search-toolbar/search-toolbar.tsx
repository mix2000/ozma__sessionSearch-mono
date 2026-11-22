import { Checkbox, Input, type InputRef } from 'antd';
import { useRef } from 'react';

import { SimpleSelect } from 'common/components';
import { useFocusShortcut } from 'common/hooks';

import { PROVIDERS } from '../../../transcripts.consts';
import type { ISearchController } from '../../models';

import { SEARCH_TOOLBAR_TEXTS as T } from './search-toolbar.consts';
import styles from './search-toolbar.module.scss';

const PROVIDER_OPTIONS = [
  { value: '', label: T.allProviders },
  ...PROVIDERS.map(p => ({ value: p.value, label: p.label })),
];

interface ISearchToolbarProps {
  /* Состояние поиска страницы (поле, фильтры, «искать сразу»). */
  state: ISearchController;
  loading: boolean;
}

/** Строка поиска, выбор агента и автоматические запуски. */
export function SearchToolbar({ state, loading }: ISearchToolbarProps) {
  const searchRef = useRef<InputRef>(null);
  useFocusShortcut(() => searchRef.current?.focus({ cursor: 'all' }));

  return (
    <div className={styles.toolbar}>
      <Input.Search
        ref={searchRef}
        className={styles.search}
        allowClear
        autoFocus
        size="large"
        placeholder={T.placeholder}
        aria-label={T.placeholder}
        value={state.input}
        onChange={e => state.setInput(e.target.value)}
        onSearch={state.submit}
        loading={loading}
      />
      <SimpleSelect
        value={state.provider}
        aria-label={T.allProviders}
        onChange={state.setProvider}
        className={styles.providerSelect}
        options={PROVIDER_OPTIONS}
      />
      <Checkbox
        checked={state.includeHeadless}
        onChange={e => state.setIncludeHeadless(e.target.checked)}
      >
        {T.headless}
      </Checkbox>
    </div>
  );
}
