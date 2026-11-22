import { splitByTerms } from '../../utils/highlight';

import styles from './highlighted-text.module.scss';

interface IHighlightedTextProps {
  text: string;
  terms: string[];
}

/** Текст с подсветкой терминов поиска. */
export function HighlightedText({ text, terms }: IHighlightedTextProps) {
  return (
    <>
      {splitByTerms(text, terms).map((part, i) =>
        part.hit ? (
          <mark key={i} className={styles.mark}>
            {part.text}
          </mark>
        ) : (
          <span key={i}>{part.text}</span>
        )
      )}
    </>
  );
}
