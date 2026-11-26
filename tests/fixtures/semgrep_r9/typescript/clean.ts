// The directives below are text, not comments: none of them is a finding.
export const notes = "// @ts-ignore and /* @ts-expect-error */";
export const template = `// @ts-nocheck`;
export const single = '/* @ts-ignore */';

export function h(x: string): number {
  return x.length;
}
