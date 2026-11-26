import { describe, expect, it } from "vitest";

const rows = [1, 2, 3];
const page = { skip: (n: number) => rows.slice(n), only: (n: number) => rows[n] };

describe("paging", () => {
  it("skips rows", () => expect(page.skip(1)).toEqual([2, 3]));
  it("names one row", () => expect(page.only(0)).toBe(1));
  it("mentions it.skip in a string", () => expect("it.skip(x)").toContain("skip"));
});
