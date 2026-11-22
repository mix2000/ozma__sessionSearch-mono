import { APP_NAME } from 'common/consts';
import { plural } from 'common/utils';

export const SEARCH_TEXTS = {
  title: APP_NAME,
  subtitle:
    'Текст всех разговоров Claude, Codex и Grok на этом компьютере: ваши запросы и ответы агента. Найдите, где обсуждали, и продолжите разговор.',
  minChars: (n: number) => `Введите хотя бы ${n} ${plural(n, ['символ', 'символа', 'символов'])}`,
  searchFailed: 'Не удалось выполнить поиск',
};
