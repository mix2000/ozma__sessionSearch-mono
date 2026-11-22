// Общие типы транспорта: метод и query-параметры запроса.

export type HttpMethod = 'GET' | 'POST';

export type Query = Record<string, string | number | undefined | null>;

export interface RequestOptions {
  params?: Query;
  body?: unknown;
}
