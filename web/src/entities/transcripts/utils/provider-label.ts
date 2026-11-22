import { PROVIDER_LABELS } from '../transcripts.consts';

/** Название агента для тегов: «Claude», неизвестный — как есть. */
export function providerLabel(provider: string): string {
  return PROVIDER_LABELS[provider] ?? provider;
}
