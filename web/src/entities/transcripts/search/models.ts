/* Состояние поиска в адресной строке (?q=&provider=&headless=1). */
export interface ISearchState {
  q: string;
  provider: string;
  includeHeadless: boolean;
}

/* Состояние страницы поиска: поле ввода, запрос, фильтры. */
export interface ISearchController extends ISearchState {
  input: string;
  setInput: (value: string) => void;
  /* Enter / кнопка поиска: искать сразу, без задержки. */
  submit: (value: string) => void;
  setProvider: (value: string) => void;
  setIncludeHeadless: (value: boolean) => void;
}

/* Тексты строки состояния индекса. */
export interface IStatusLineTexts {
  stats: (sessions: number, messages: number) => string;
  updated: (ago: string) => string;
  indexing: string;
}
