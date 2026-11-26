// @ts-nocheck
export function f(x: unknown): number {
  // @ts-ignore
  return x.length;
}

export function g(x: unknown): number {
  /* @ts-expect-error the type is wrong */
  return x.size;
}

export function k(x: unknown): number {
  /** @ts-ignore */
  return x.n;
}

export function m(x: unknown): number {
  /**
   * @ts-expect-error the last line of a block comment counts */
  return x.n;
}
