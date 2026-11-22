import { describe, expect, it } from "vitest";

import { buildSearch, parseSearchState } from "./url-state";

describe("url-state", () => {
  it("читает запрос из адреса", () => {
    expect(parseSearchState("?q=%20nginx%20&provider=grok&headless=1")).toEqual({
      q: "nginx",
      provider: "grok",
      includeHeadless: true,
    });
    expect(parseSearchState("?provider=evil")).toEqual({
      q: "",
      provider: "",
      includeHeadless: false,
    });
  });

  it("пишет запрос в адрес, не трогая чужие параметры", () => {
    expect(
      buildSearch("?x=1&q=old", { q: "a b", provider: "", includeHeadless: false }),
    ).toBe("?x=1&q=a+b");
    expect(
      buildSearch("?q=old", { q: "", provider: "", includeHeadless: false }),
    ).toBe("");
  });
});
