// Публичный API транспорта: сервис запросов, ключи кэша, ошибки для показа.
// Сам запрос (request) и разбор тела ошибки наружу не отдаются.
export { NETWORK_ERROR_MESSAGE } from './api.consts';
export { ApiError, apiErrorMessage } from './api.errors';
export { captureTokenFromUrl } from './api.instance';
export { apiService } from './api.service';
export type { HttpMethod, Query, RequestOptions } from './api.types';
export { queryKeys } from './query-keys';
