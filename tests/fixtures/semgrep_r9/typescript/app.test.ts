import { describe, expect, it, test } from "vitest";

describe.skip("parser", () => {
  it("reads", () => expect(1).toBe(1));
});

it.skip("writes", () => {});
test.skip("deletes", () => {});
xit("an old one", () => {});
xdescribe("an old suite", () => {});
xtest("an older one", () => {});
it.skipIf(process.env.CI)("off on CI", () => {});
test.skip.each([1, 2])("n %i", () => {});

describe.only("focused", () => {});
it.only("this one", () => {});
test.only.each([1])("n %i", () => {});
fit("jasmine focus", () => {});
fdescribe("jasmine focus suite", () => {});
