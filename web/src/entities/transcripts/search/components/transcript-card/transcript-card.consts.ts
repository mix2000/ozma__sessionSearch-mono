export const TRANSCRIPT_CARD_TEXTS = {
  untitled: 'Без названия',
  more: (n: number) => `ещё совпадений: ${n}`,
  messages: (n: number) => `${n} реплик`,
  headlessTag: 'автоматический',
  liveExternal: (pid: number) => `открыт в терминале (pid ${pid})`,
  workdirUnknown: 'папка неизвестна',
};

/** Цвет тега «открыт в терминале». */
export const LIVE_TAG_COLOR = 'orange';
