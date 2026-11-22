import { describe, expect, it } from "vitest";

import { folderName } from "./format";
import { quoteDir, resumeCommand, splitByTerms } from "./highlight";

describe("splitByTerms", () => {
  it("подсвечивает термины без учёта регистра", () => {
    expect(splitByTerms("Деплой nginx и NGINX", ["nginx"])).toEqual([
      { text: "Деплой ", hit: false },
      { text: "nginx", hit: true },
      { text: " и ", hit: false },
      { text: "NGINX", hit: true },
    ]);
  });

  it("спецсимволы регэкспа в запросе безопасны", () => {
    expect(splitByTerms("a.b (x)", ["(x)"])).toEqual([
      { text: "a.b ", hit: false },
      { text: "(x)", hit: true },
    ]);
  });

  it("без терминов — весь текст одним куском", () => {
    expect(splitByTerms("abc", ["", "  "])).toEqual([
      { text: "abc", hit: false },
    ]);
  });

  it("не меняет переданный массив терминов", () => {
    const terms = ["a", "abc"];
    splitByTerms("abc", terms);
    expect(terms).toEqual(["a", "abc"]);
  });
});

describe("resumeCommand", () => {
  it("claude и codex", () => {
    expect(resumeCommand("claude", "id1", "/w")).toBe(
      'cd "/w" && claude --resume id1',
    );
    expect(resumeCommand("codex", "id2", null)).toBe("codex resume id2");
    expect(resumeCommand("codex", "id3", "/w")).toBe(
      'cd "/w" && codex resume id3',
    );
  });

  it("Windows-путь остаётся как есть", () => {
    expect(resumeCommand("claude", "id", "C:\\Users\\me\\repo")).toBe(
      'cd "C:\\Users\\me\\repo" && claude --resume id',
    );
  });

  it("grok — команду знает только бэкенд", () => {
    expect(resumeCommand("grok", "id", "/w")).toBeNull();
  });
});

describe("quoteDir", () => {
  it("опасные для двойных кавычек символы — одинарные кавычки", () => {
    expect(quoteDir("/tmp/$HOME")).toBe("'/tmp/$HOME'");
    expect(quoteDir("/tmp/it's")).toBe('"/tmp/it\'s"');
    expect(quoteDir("/tmp/a'$b")).toBe("'/tmp/a'\\''$b'");
  });
});

describe("folderName", () => {
  it("последний сегмент пути, домашняя — ~", () => {
    expect(folderName("/Users/me/work/repo/", "?")).toBe("repo");
    expect(folderName("/Users/me", "?")).toBe("~");
    expect(folderName("/home/me/", "?")).toBe("~");
    expect(folderName("C:\\Users\\me", "?")).toBe("~");
    expect(folderName("D:\\work\\repo", "?")).toBe("repo");
    expect(folderName(null, "?")).toBe("?");
  });
});
