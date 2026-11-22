import { useMediaQuery } from './use-media-query';

const TOUCH_QUERY = '(pointer: coarse)';

/** Тач-устройство (грубый указатель). */
export function useIsTouch(): boolean {
  return useMediaQuery(TOUCH_QUERY);
}
