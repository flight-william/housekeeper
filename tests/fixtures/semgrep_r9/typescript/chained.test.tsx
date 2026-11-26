import { describe, it, test } from "vitest";

it.concurrent.skip("a", () => {});
describe.concurrent.skip("b", () => {});
it.skip.concurrent("c", () => {});
test.concurrent.skip.each([1])("d %i", () => {});
test.skip.failing("e", () => {});
specify.skip("f", () => {});
xcontext("g", () => {});
xspecify("h", () => {});

it.concurrent.only("i", () => {});
test.concurrent.only.each([1])("j %i", () => {});
test.only.failing("k", () => {});
test.fails.only("l", () => {});

it.concurrent("runs", () => <div>ok</div>);
