import { describe, expect, it } from 'vitest';

import { ApiError, apiErrorMessage, parseErrorBody } from './api.errors';

describe('parseErrorBody', () => {
  it('message из JSON, массив склеивается', () => {
    expect(parseErrorBody('{"statusCode":400,"message":"плохо"}', 400)).toBe('плохо');
    expect(parseErrorBody('{"message":["a","b"]}', 400)).toBe('a; b');
    expect(parseErrorBody('{"error":"Not Found"}', 404)).toBe('Not Found');
  });

  it('не JSON — текст или HTTP-статус', () => {
    expect(parseErrorBody(' oops ', 500)).toBe('oops');
    expect(parseErrorBody('', 502)).toBe('HTTP 502');
  });
});

describe('apiErrorMessage', () => {
  it('сообщение ApiError или запасной текст', () => {
    expect(apiErrorMessage(new ApiError(409, 'занят'), 'x')).toBe('занят');
    expect(apiErrorMessage(new Error('boom'), 'x')).toBe('x');
  });
});
