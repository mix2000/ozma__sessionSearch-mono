// Тонкий клиент API утилиты: тот же origin, без логина. Необязательный режим
// с токеном: страницу открыли как ?token=… — токен запоминается в
// sessionStorage, убирается из адресной строки и уходит в Authorization.

const TOKEN_KEY = "session-search-token";
const API_BASE = "/api";

let memoryToken: string | null = null;

function readStoredToken(): string | null {
  try {
    return sessionStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

/** Забрать ?token= из адреса (один раз при старте) и убрать его из URL. */
export function captureTokenFromUrl(): void {
  const url = new URL(window.location.href);
  const token = url.searchParams.get("token");
  if (!token) return;
  memoryToken = token;
  try {
    sessionStorage.setItem(TOKEN_KEY, token);
  } catch {
    // приватный режим — токен живёт только в памяти вкладки
  }
  url.searchParams.delete("token");
  window.history.replaceState(null, "", url.pathname + url.search + url.hash);
}

function authToken(): string | null {
  return memoryToken ?? readStoredToken();
}

export class ApiError extends Error {
  readonly status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

type Query = Record<string, string | number | undefined | null>;

async function errorMessage(res: Response): Promise<string> {
  const text = await res.text().catch(() => "");
  try {
    const body = JSON.parse(text) as { message?: string | string[]; error?: string };
    const msg = body.message ?? body.error;
    if (Array.isArray(msg)) return msg.join("; ");
    if (msg) return msg;
  } catch {
    // не JSON
  }
  return text.trim() || `HTTP ${res.status}`;
}

export async function request<T>(
  method: "GET" | "POST",
  path: string,
  options: { params?: Query; body?: unknown } = {},
): Promise<T> {
  const url = new URL(API_BASE + path, window.location.origin);
  for (const [k, v] of Object.entries(options.params ?? {})) {
    if (v !== undefined && v !== null && v !== "") url.searchParams.set(k, String(v));
  }
  const headers: Record<string, string> = { Accept: "application/json" };
  const token = authToken();
  if (token) headers.Authorization = `Bearer ${token}`;
  if (options.body !== undefined) headers["Content-Type"] = "application/json";
  let res: Response;
  try {
    res = await fetch(url, {
      method,
      headers,
      body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
    });
  } catch {
    throw new ApiError(0, "Сервер поиска не отвечает");
  }
  if (!res.ok) throw new ApiError(res.status, await errorMessage(res));
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

/** Текст ошибки для пользователя: сообщение сервера или запасной текст. */
export function apiErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof ApiError && error.message) return error.message;
  return fallback;
}
