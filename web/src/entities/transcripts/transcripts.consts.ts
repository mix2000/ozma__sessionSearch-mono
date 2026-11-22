import type { IProviderColors, IProviderOption } from './models';

export const PROVIDERS: IProviderOption[] = [
  { value: 'claude', label: 'Claude' },
  { value: 'codex', label: 'Codex' },
  { value: 'grok', label: 'Grok' },
];

export const KNOWN_PROVIDERS: string[] = PROVIDERS.map(p => p.value);

/** Подписи агентов по коду. */
export const PROVIDER_LABELS: Record<string, string> = Object.fromEntries(
  PROVIDERS.map(p => [p.value, p.label])
);

/** Цвета тегов агента: в шапке карточки (`agent`) и у его реплик (`reply`). */
export const PROVIDER_COLORS: Record<string, IProviderColors> = {
  claude: { agent: 'purple', reply: 'purple' },
  codex: { agent: 'blue', reply: 'geekblue' },
  grok: { agent: 'cyan', reply: 'cyan' },
};

/** Задержка поиска после ввода, мс. */
export const SEARCH_DEBOUNCE_MS = 400;
