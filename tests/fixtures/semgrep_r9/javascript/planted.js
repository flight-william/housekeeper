/* eslint-disable */
export function log(x) {
  // eslint-disable-next-line no-console
  console.log(x);
  console.warn(x); // eslint-disable-line no-console
  /* eslint-disable-next-line no-alert */
  alert(x);
  // @ts-ignore
  return x.y;
}
