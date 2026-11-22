// Мок API поиска для разработки UI без бэкенда. Только выдуманные данные.
// Запуск: MOCK_PORT=4478 node mock/mock-api.mjs
// и в другом терминале: SESSION_SEARCH_API=http://127.0.0.1:4478 npm run dev -- --port 5180
import http from 'node:http';

const PORT = Number(process.env.MOCK_PORT ?? 4478);
const now = Date.now();
const ago = min => new Date(now - min * 60_000).toISOString();

const sessions = [
  {
    id: '0b6f1d2e-1111-4a5b-9c3d-aaaaaaaaaaaa',
    provider: 'claude',
    title: 'Настроить nginx для демо-стенда и проверить редиректы',
    workdir: '/Users/demo/work/demo-shop',
    kind: 'interactive',
    gitBranch: 'feature/nginx-redirects',
    startedAt: ago(600),
    lastAt: ago(35),
    workdirExists: true,
    live: { kind: 'external', pid: 48213 },
    resumeCommand:
      'cd /Users/demo/work/demo-shop && claude --resume 0b6f1d2e-1111-4a5b-9c3d-aaaaaaaaaaaa',
  },
  {
    id: '019a2b3c-2222-7def-8123-bbbbbbbbbbbb',
    provider: 'codex',
    title: 'Почему nginx отдаёт 502 после деплоя',
    workdir: 'C:\\Users\\demo\\projects\\sample-api',
    kind: 'interactive',
    gitBranch: 'main',
    startedAt: ago(3000),
    lastAt: ago(2900),
    workdirExists: true,
    resumeCommand:
      'cd /d "C:\\Users\\demo\\projects\\sample-api" && codex resume 019a2b3c-2222-7def-8123-bbbbbbbbbbbb',
  },
  {
    id: 'grok-3c4d5e6f-3333-4abc-9def-cccccccccccc',
    provider: 'grok',
    title: 'Переписать конфиг nginx на шаблоны',
    workdir: '/home/demo/sandbox/landing',
    kind: 'interactive',
    gitBranch: null,
    startedAt: ago(9000),
    lastAt: ago(8800),
    workdirExists: true,
    resumeCommand: 'cd "/home/demo/sandbox/landing" && grok --resume grok-3c4d5e6f',
  },
  {
    id: '0c7a8b9d-4444-4e5f-8a6b-dddddddddddd',
    provider: 'claude',
    title: null,
    workdir: '/Users/demo/old/removed-project-with-a-very-long-folder-name-that-should-wrap',
    kind: 'headless',
    gitBranch: 'chore/very-long-branch-name-to-check-wrapping-on-phones',
    startedAt: ago(40000),
    lastAt: ago(39000),
    workdirExists: false,
    resumeCommand:
      'cd /Users/demo/old/removed-project-with-a-very-long-folder-name-that-should-wrap && claude --resume 0c7a8b9d-4444-4e5f-8a6b-dddddddddddd',
  },
];

function messagesFor(s) {
  const agent = s.provider;
  return Array.from({ length: 12 }, (_, i) => ({
    id: i + 1,
    seq: i,
    role: i % 2 === 0 ? 'user' : agent,
    at: ago(Math.max(1, 600 - i * 10)),
    text:
      i % 2 === 0
        ? `Вопрос ${i / 2 + 1}: посмотри конфиг NGINX и скажи, почему location /api не срабатывает.`
        : `Ответ ${Math.ceil(i / 2)}: в конфиге nginx блок location /api стоит после location /, поэтому запросы уходят не туда.\n\nИсправление:\n  location /api/ {\n    proxy_pass http://127.0.0.1:3000/;\n  }\nПосле правки — nginx -t и перезагрузка.`,
  }));
}

function search(q, provider, headless) {
  const terms = [...q.matchAll(/"([^"]+)"|(\S+)/g)].map(m => m[1] ?? m[2]);
  const low = terms.map(t => t.toLowerCase());
  const results = sessions
    .filter(s => !provider || s.provider === provider)
    .filter(s => headless || s.kind !== 'headless')
    .map(s => {
      const msgs = messagesFor(s).filter(m => low.every(t => m.text.toLowerCase().includes(t)));
      return {
        ...s,
        messageCount: 12,
        matches: msgs.length,
        hits: msgs.slice(0, 3).map(m => ({
          messageId: m.id,
          seq: m.seq,
          role: m.role,
          at: m.at,
          snippet: {
            before: m.seq > 0 ? 'Предыдущий абзац для контекста.' : undefined,
            match: m.text.split('\n')[0],
            after: 'Следующий абзац после совпадения.',
          },
        })),
      };
    })
    .filter(r => r.matches > 0);
  return { terms, results, truncated: false };
}

function send(res, code, body) {
  res.writeHead(code, { 'Content-Type': 'application/json; charset=utf-8' });
  res.end(JSON.stringify(body));
}

const server = http.createServer((req, res) => {
  const url = new URL(req.url ?? '/', 'http://localhost');
  const p = url.pathname;
  if (p === '/api/agents/transcripts/status') {
    return send(res, 200, {
      sessions: 1234,
      messages: 56789,
      minQueryLength: 2,
      index: { running: false, lastRunAt: ago(3), lastDurationMs: 42, lastError: null },
      providers: [
        { provider: 'claude', root: '/Users/demo/.claude/projects', exists: true, sessions: 812 },
        { provider: 'codex', root: '/Users/demo/.codex/sessions', exists: true, sessions: 400 },
        { provider: 'grok', root: '/Users/demo/.grok/sessions', exists: false, sessions: 0 },
      ],
    });
  }
  if (p === '/api/agents/transcripts/sync' && req.method === 'POST') {
    return send(res, 200, { files: 3, updated: 2, messages: 17 });
  }
  if (p === '/api/agents/transcripts/search') {
    return send(
      res,
      200,
      search(
        url.searchParams.get('q') ?? '',
        url.searchParams.get('provider'),
        url.searchParams.get('includeHeadless') === 'true'
      )
    );
  }
  let m = p.match(/^\/api\/agents\/transcripts\/([^/]+)\/context$/);
  if (m) {
    const s = sessions.find(x => x.id === decodeURIComponent(m[1]));
    if (!s) return send(res, 404, { message: 'Разговор не найден' });
    const seq = Number(url.searchParams.get('seq') ?? 0);
    return send(
      res,
      200,
      messagesFor(s).filter(x => Math.abs(x.seq - seq) <= 3)
    );
  }
  m = p.match(/^\/api\/agents\/transcripts\/([^/]+)\/resume$/);
  if (m && req.method === 'POST') {
    const s = sessions.find(x => x.id === decodeURIComponent(m[1]));
    if (!s) return send(res, 404, { message: 'Разговор не найден' });
    if (s.provider === 'codex')
      return send(res, 409, {
        message: 'Папки разговора нет: C:\\Users\\demo\\projects\\sample-api',
      });
    return send(res, 200, {
      ok: true,
      command: s.resumeCommand,
    });
  }
  send(res, 404, { message: 'not found' });
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`mock API on http://127.0.0.1:${PORT}`);
});
