// @ts-nocheck
export function f(x: unknown): number {
  // @ts-ignore
  return x.length;
}

export function g(x: unknown): number {
  /* @ts-expect-error the type is wrong */
  return x.size;
}
