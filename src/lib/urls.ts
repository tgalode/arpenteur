export function normalizeUrl(u: string): string {
  try {
    const url = new URL(u);
    url.hash = "";
    return url.href;
  } catch {
    return u;
  }
}
