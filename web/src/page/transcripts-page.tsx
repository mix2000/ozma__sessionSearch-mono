import {
  FolderOpenOutlined,
  PlayCircleOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import {
  App as AntApp,
  Button,
  Card,
  Checkbox,
  Empty,
  Input,
  Modal,
  Popover,
  Skeleton,
  Space,
  Spin,
  Tag,
  Tooltip,
  Typography,
  type InputRef,
} from "antd";
import { useEffect, useRef, useState } from "react";

import { apiErrorMessage } from "../api/client";
import {
  useResumeTranscript,
  useSyncTranscripts,
  useTranscriptContext,
  useTranscriptSearch,
  useTranscriptStatus,
} from "../api/hooks";
import type {
  ProviderSource,
  TranscriptHit,
  TranscriptResult,
} from "../api/models";
import { PageHeader } from "../components/page-header";
import { SimpleSelect } from "../components/select";
import { folderName, formatRelative, formatShortDate } from "./format";
import { resumeCommand, splitByTerms } from "./highlight";
import {
  PROVIDERS,
  PROVIDER_TAG_COLOR,
  ROLE_TAG_COLOR,
  TRANSCRIPTS_TEXTS as T,
} from "./transcripts-page.consts";
import styles from "./transcripts-page.module.scss";
import { buildSearch, parseSearchState } from "./url-state";

function Highlighted({ text, terms }: { text: string; terms: string[] }) {
  return (
    <>
      {splitByTerms(text, terms).map((part, i) =>
        part.hit ? (
          <mark key={i} className={styles.mark}>
            {part.text}
          </mark>
        ) : (
          <span key={i}>{part.text}</span>
        ),
      )}
    </>
  );
}

function roleLabel(role: string, provider: string) {
  return role === "user" ? (
    <Tag color="blue">{T.you}</Tag>
  ) : (
    <Tag color={ROLE_TAG_COLOR[provider] ?? "purple"}>{T.agent(provider)}</Tag>
  );
}

interface ContextTarget {
  result: TranscriptResult;
  seq: number;
}

function ContextModal({
  target,
  terms,
  onClose,
}: {
  target: ContextTarget | null;
  terms: string[];
  onClose: () => void;
}) {
  const ctx = useTranscriptContext(
    target?.result.id ?? "",
    target?.seq ?? null,
  );
  return (
    <Modal
      open={!!target}
      onCancel={onClose}
      footer={null}
      width={820}
      title={target?.result.title ?? T.contextTitle}
    >
      {ctx.isLoading && <Skeleton active />}
      {ctx.isError && (
        <Typography.Text type="danger">
          {apiErrorMessage(ctx.error, T.contextFailed)}
        </Typography.Text>
      )}
      {ctx.data?.map((m) => (
        <div
          key={m.id}
          className={`${styles.contextMsg} ${m.seq === target?.seq ? styles.contextCurrent : ""}`}
        >
          <div className={styles.hitHead}>
            {roleLabel(m.role, target?.result.provider ?? "claude")}
            {m.at && (
              <Typography.Text type="secondary">
                {formatShortDate(m.at)}
              </Typography.Text>
            )}
          </div>
          <pre className={styles.text}>
            <Highlighted text={m.text} terms={terms} />
          </pre>
        </div>
      ))}
    </Modal>
  );
}

function HitView({
  hit,
  result,
  terms,
  onContext,
}: {
  hit: TranscriptHit;
  result: TranscriptResult;
  terms: string[];
  onContext: () => void;
}) {
  return (
    <div className={styles.hit}>
      <div className={styles.hitHead}>
        {roleLabel(hit.role, result.provider)}
        {hit.at && (
          <Typography.Text type="secondary">
            {formatShortDate(hit.at)}
          </Typography.Text>
        )}
        <Button size="small" type="link" onClick={onContext}>
          {T.context}
        </Button>
      </div>
      {hit.snippet.before && (
        <pre className={`${styles.text} ${styles.dim}`}>
          {hit.snippet.before}
        </pre>
      )}
      <pre className={styles.text}>
        <Highlighted text={hit.snippet.match} terms={terms} />
      </pre>
      {hit.snippet.after && (
        <pre className={`${styles.text} ${styles.dim}`}>
          {hit.snippet.after}
        </pre>
      )}
    </div>
  );
}

function ResultActions({ result }: { result: TranscriptResult }) {
  const { message } = AntApp.useApp();
  const resume = useResumeTranscript();
  // Команда от бэкенда в приоритете; для grok другой и нет.
  const [serverCommand, setServerCommand] = useState<string | null>(null);
  const command =
    result.resumeCommand ??
    serverCommand ??
    resumeCommand(result.provider, result.id, result.workdir);

  async function doResume() {
    try {
      const res = await resume.mutateAsync(result.id);
      if (res?.command) setServerCommand(res.command);
      message.success(T.resumed);
    } catch (err) {
      message.error(apiErrorMessage(err, T.resumeFailed));
    }
  }

  let main;
  if (result.live?.kind === "external") {
    main = <Typography.Text type="secondary">{T.alreadyOpen}</Typography.Text>;
  } else if (!result.workdirExists) {
    main = <Typography.Text type="secondary">{T.noFolder}</Typography.Text>;
  } else {
    main = (
      <Button
        type="primary"
        icon={<PlayCircleOutlined />}
        loading={resume.isPending}
        onClick={doResume}
      >
        {T.resume}
      </Button>
    );
  }

  return (
    <div className={styles.actions}>
      {main}
      {command && (
        <Tooltip title={T.copyCommand}>
          <Typography.Text code copyable className={styles.command}>
            {command}
          </Typography.Text>
        </Tooltip>
      )}
    </div>
  );
}

function ResultCard({
  result,
  terms,
  onContext,
}: {
  result: TranscriptResult;
  terms: string[];
  onContext: (seq: number) => void;
}) {
  const place =
    result.projectName && result.repoName
      ? `${result.projectName} / ${result.repoName}`
      : folderName(result.workdir, T.workdirUnknown);
  return (
    <Card
      size="small"
      className={styles.card}
      title={
        <span className={styles.cardTitle}>
          {result.title || T.untitled}
          {result.live?.kind === "external" && (
            <Tag color="orange">{T.liveExternal(result.live.pid)}</Tag>
          )}
        </span>
      }
    >
      <div className={styles.meta}>
        <Tag color={PROVIDER_TAG_COLOR[result.provider] ?? "default"}>
          {T.agent(result.provider)}
        </Tag>
        {result.kind === "headless" && <Tag>{T.headlessTag}</Tag>}
        <Typography.Text strong>{place}</Typography.Text>
        {result.gitBranch && <Tag>{result.gitBranch}</Tag>}
        <Typography.Text type="secondary">
          {result.lastAt ? formatShortDate(result.lastAt) : ""} ·{" "}
          {T.messages(result.messageCount)}
        </Typography.Text>
      </div>
      {result.workdir && (
        <Typography.Text type="secondary" className={styles.path}>
          {result.workdir}
        </Typography.Text>
      )}
      {result.hits.map((hit) => (
        <HitView
          key={hit.messageId}
          hit={hit}
          result={result}
          terms={terms}
          onContext={() => onContext(hit.seq)}
        />
      ))}
      {result.matches > result.hits.length && (
        <Typography.Text type="secondary">
          {T.more(result.matches - result.hits.length)}
        </Typography.Text>
      )}
      <ResultActions result={result} />
    </Card>
  );
}

function SourcesList({ sources }: { sources: ProviderSource[] }) {
  return (
    <div className={styles.sources}>
      {sources.map((s) => (
        <div
          key={`${s.provider}:${s.root}`}
          className={`${styles.source} ${s.exists ? "" : styles.sourceMissing}`}
        >
          <div className={styles.sourceHead}>
            <Tag color={s.exists ? PROVIDER_TAG_COLOR[s.provider] : undefined}>
              {T.agent(s.provider)}
            </Tag>
            <Typography.Text type="secondary">
              {s.exists ? T.sourceSessions(s.sessions) : T.sourceMissing}
            </Typography.Text>
          </div>
          <Typography.Text code className={styles.path}>
            {s.root}
          </Typography.Text>
        </div>
      ))}
    </div>
  );
}

function isTypingTarget(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  return (
    el.isContentEditable ||
    el.tagName === "INPUT" ||
    el.tagName === "TEXTAREA" ||
    el.tagName === "SELECT"
  );
}

/** Полнотекстовый поиск по разговорам агентов и продолжение. */
export function TranscriptsPage() {
  const { message } = AntApp.useApp();
  const [initial] = useState(() => parseSearchState(window.location.search));
  const [input, setInput] = useState(initial.q);
  const [q, setQ] = useState(initial.q);
  const [provider, setProvider] = useState(initial.provider);
  const [includeHeadless, setIncludeHeadless] = useState(
    initial.includeHeadless,
  );
  const [context, setContext] = useState<ContextTarget | null>(null);
  const searchRef = useRef<InputRef>(null);

  useEffect(() => {
    const t = setTimeout(() => setQ(input.trim()), 400);
    return () => clearTimeout(t);
  }, [input]);

  // Запрос — в адресную строку (replaceState: без лишних шагов «назад»).
  useEffect(() => {
    const search = buildSearch(window.location.search, {
      q,
      provider,
      includeHeadless,
    });
    if (search !== window.location.search) {
      window.history.replaceState(
        null,
        "",
        window.location.pathname + search + window.location.hash,
      );
    }
  }, [q, provider, includeHeadless]);

  // «/» или Cmd/Ctrl+K — к полю поиска.
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const cmdK =
        (e.metaKey || e.ctrlKey) && !e.altKey && e.key.toLowerCase() === "k";
      const slash =
        e.key === "/" &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey &&
        !isTypingTarget(e.target);
      if (!cmdK && !slash) return;
      e.preventDefault();
      searchRef.current?.focus({ cursor: "all" });
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const status = useTranscriptStatus();
  const sync = useSyncTranscripts();
  const search = useTranscriptSearch({ q, provider, includeHeadless });
  const terms = search.data?.terms ?? [];
  const results = search.data?.results ?? [];

  async function handleSync() {
    try {
      const res = await sync.mutateAsync();
      message.success(T.refreshed(res.updated));
    } catch (err) {
      message.error(apiErrorMessage(err, T.refresh));
    }
  }

  const index = status.data?.index;
  const sources = status.data?.providers ?? [];
  return (
    <div className={styles.page}>
      <Space orientation="vertical" size="middle" style={{ width: "100%" }}>
        <PageHeader title={T.title} subtitle={T.subtitle} />
        <Card size="small">
          <div className={styles.toolbar}>
            <Input.Search
              ref={searchRef}
              className={styles.search}
              allowClear
              autoFocus
              size="large"
              placeholder={T.placeholder}
              aria-label={T.placeholder}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onSearch={(v) => {
                setInput(v);
                setQ(v.trim());
              }}
              loading={search.isFetching}
            />
            <SimpleSelect
              value={provider}
              aria-label={T.allProviders}
              onChange={setProvider}
              className={styles.providerSelect}
              options={[
                { value: "", label: T.allProviders },
                ...PROVIDERS.map((p) => ({ value: p.value, label: p.label })),
              ]}
            />
            <Checkbox
              checked={includeHeadless}
              onChange={(e) => setIncludeHeadless(e.target.checked)}
            >
              {T.headless}
            </Checkbox>
          </div>
          <div className={styles.status}>
            {status.data && (
              <Typography.Text type="secondary">
                {T.stats(status.data.sessions, status.data.messages)}
                {index?.running
                  ? ` · ${T.indexing}`
                  : index?.lastRunAt
                    ? ` · ${T.updated(formatRelative(index.lastRunAt))}`
                    : ""}
              </Typography.Text>
            )}
            {status.isError && (
              <Typography.Text type="danger">
                {apiErrorMessage(status.error, T.statusFailed)}
              </Typography.Text>
            )}
            {index?.lastError && (
              <Typography.Text type="danger">{index.lastError}</Typography.Text>
            )}
            <Button
              size="small"
              icon={<ReloadOutlined />}
              loading={sync.isPending}
              onClick={handleSync}
            >
              {T.refresh}
            </Button>
            {sources.length > 0 && (
              <Popover
                trigger="click"
                title={T.sourcesTitle}
                content={<SourcesList sources={sources} />}
              >
                <Button size="small" icon={<FolderOpenOutlined />}>
                  {T.sources}
                </Button>
              </Popover>
            )}
            <Typography.Text type="secondary" className={styles.hint}>
              {T.searchHint}
            </Typography.Text>
          </div>
        </Card>

        {q.length > 0 && q.length < 2 && (
          <Typography.Text type="secondary">{T.minChars}</Typography.Text>
        )}
        {search.isLoading && <Spin />}
        {search.isError && (
          <Typography.Text type="danger">
            {apiErrorMessage(search.error, T.nothing)}
          </Typography.Text>
        )}
        {search.data && q.length >= 2 && (
          <>
            {results.length === 0 ? (
              <Empty description={T.nothing}>
                <Typography.Text type="secondary">
                  {T.nothingHint}
                </Typography.Text>
              </Empty>
            ) : (
              <Typography.Text type="secondary">
                {T.found(results.length, search.data.truncated)}
              </Typography.Text>
            )}
            {results.map((r) => (
              <ResultCard
                key={r.id}
                result={r}
                terms={terms}
                onContext={(seq) => setContext({ result: r, seq })}
              />
            ))}
          </>
        )}

        <ContextModal
          target={context}
          terms={terms}
          onClose={() => setContext(null)}
        />
      </Space>
    </div>
  );
}
