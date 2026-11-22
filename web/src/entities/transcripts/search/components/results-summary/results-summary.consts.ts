export const RESULTS_SUMMARY_TEXTS = {
  found: (n: number, truncated: boolean) =>
    `Разговоров: ${n}${truncated ? ' (показаны самые свежие)' : ''}`,
};
