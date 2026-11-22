import { Button } from 'antd';

import { MessageHead } from '../../../components/message-head';
import { MessageText } from '../../../components/message-text';
import type { ITranscriptHit } from '../../../models';

import { HIT_SNIPPET_TEXTS as T } from './hit-snippet.consts';
import styles from './hit-snippet.module.scss';

interface IHitSnippetProps {
  hit: ITranscriptHit;
  provider: string;
  terms: string[];
  onContext: () => void;
}

/** Фрагмент совпадения: абзац с подсветкой и соседние абзацы. */
export function HitSnippet({ hit, provider, terms, onContext }: IHitSnippetProps) {
  return (
    <div className={styles.hit}>
      <MessageHead
        role={hit.role}
        provider={provider}
        at={hit.at}
        extra={
          <Button size="small" type="link" onClick={onContext}>
            {T.context}
          </Button>
        }
      />
      {hit.snippet.before && <MessageText text={hit.snippet.before} dim />}
      <MessageText text={hit.snippet.match} terms={terms} />
      {hit.snippet.after && <MessageText text={hit.snippet.after} dim />}
    </div>
  );
}
