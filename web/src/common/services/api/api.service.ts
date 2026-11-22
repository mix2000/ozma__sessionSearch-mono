// Методы API поверх транспорта: сущности зовут их, а не fetch.

import { request } from './api.instance';

import type { Query } from './api.types';

export const apiService = {
  get<T>(path: string, params?: Query): Promise<T> {
    return request<T>('GET', path, params ? { params } : {});
  },
  post<T>(path: string, body?: unknown): Promise<T> {
    return request<T>('POST', path, body !== undefined ? { body } : {});
  },
};
