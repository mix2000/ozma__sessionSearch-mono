import { HighlightedText } from '../highlighted-text';

import styles from './message-text.module.scss';

interface IMessageTextProps {
  text: string;
  /** Термины для подсветки; без них — текст как есть. */
  terms?: string[];
  /** Приглушённый (соседние абзацы фрагмента). */
  dim?: boolean;
}

/** Текст реплики с сохранением переносов. */
export function MessageText({ text, terms, dim }: IMessageTextProps) {
  return (
    <pre className={dim ? `${styles.text} ${styles.dim}` : styles.text}>
      {terms ? <HighlightedText text={text} terms={terms} /> : text}
    </pre>
  );
}
