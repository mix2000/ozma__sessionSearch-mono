// Ошибки API: класс с HTTP-статусом и разбор тела ответа NestJS-формата
// ({statusCode, message, error}).

export class ApiError extends Error {
  readonly status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

/** Текст ошибки из тела ответа: message/error из JSON или сам текст. */
export function parseErrorBody(text: string, status: number): string {
  try {
    const body = JSON.parse(text) as { message?: string | string[]; error?: string };
    const msg = body.message ?? body.error;
    if (Array.isArray(msg)) return msg.join('; ');
    if (msg) return msg;
  } catch {
    // не JSON
  }
  return text.trim() || `HTTP ${status}`;
}

export async function errorMessage(res: Response): Promise<string> {
  const text = await res.text().catch(() => '');
  return parseErrorBody(text, res.status);
}

/** Текст ошибки для пользователя: сообщение сервера или запасной текст. */
export function apiErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof ApiError && error.message) return error.message;
  return fallback;
}
