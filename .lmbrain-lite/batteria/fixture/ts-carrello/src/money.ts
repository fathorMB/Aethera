/** Rounds to the nearest integer, halves away from zero: 2.5 -> 3, -2.5 -> -3. */
export function roundHalfUp(value: number): number {
  const rounded = Math.floor(Math.abs(value) + 0.5 + 1e-9);
  return value < 0 ? -rounded : rounded;
}

export function formatCents(cents: number): string {
  const sign = cents < 0 ? "-" : "";
  const abs = Math.abs(cents);
  return `${sign}${Math.floor(abs / 100)}.${String(abs % 100).padStart(2, "0")}`;
}
