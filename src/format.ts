const nf = new Intl.NumberFormat("it-IT");

export const num = (n: number | null | undefined) => (n == null ? null : nf.format(n));

export function duration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d > 0) return `${d} g ${h} h`;
  if (h > 0) return `${h} h ${m} m`;
  if (m > 0) return `${m} m ${s % 60} s`;
  return `${s} s`;
}

export function clock(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (x: number) => String(x).padStart(2, "0");
  return `${p(d.getDate())}-${p(d.getMonth() + 1)} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

export function getPath(obj: unknown, path: string): unknown {
  return path.split(".").reduce<unknown>((o, k) => (o == null ? undefined : (o as Record<string, unknown>)[k]), obj);
}

export function setPath<T>(obj: T, path: string, value: unknown): T {
  const copy = structuredClone(obj) as Record<string, unknown>;
  const keys = path.split(".");
  let cur = copy;
  for (const k of keys.slice(0, -1)) {
    if (cur[k] == null || typeof cur[k] !== "object") cur[k] = {};
    cur = cur[k] as Record<string, unknown>;
  }
  cur[keys[keys.length - 1]] = value;
  return copy as T;
}

/** Assente e null sono lo stesso valore: nessuno dei due è zero. */
export function sameValue(a: unknown, b: unknown): boolean {
  const norm = (x: unknown) => (x === undefined ? null : Array.isArray(x) && x.length === 0 ? null : x);
  return JSON.stringify(norm(a)) === JSON.stringify(norm(b));
}

export function show(v: unknown): string {
  if (v == null) return "—";
  if (Array.isArray(v)) return v.join(" ");
  return String(v);
}
