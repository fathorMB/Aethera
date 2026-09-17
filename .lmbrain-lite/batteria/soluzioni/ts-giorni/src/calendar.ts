// Business-day arithmetic. Every date is a Date at 00:00:00 UTC, and every computation
// uses the UTC getters and setters, never the local ones. Holidays are "YYYY-MM-DD" strings.

export function day(iso: string): Date {
  const d = new Date(`${iso}T00:00:00Z`);
  if (Number.isNaN(d.getTime())) {
    throw new Error(`not a date: ${iso}`);
  }
  return d;
}

export function isoDay(date: Date): string {
  return date.toISOString().slice(0, 10);
}

/** Saturday and Sunday. */
export function isWeekend(date: Date): boolean {
  const weekday = date.getUTCDay();
  return weekday === 0 || weekday === 6;
}

export function isBusinessDay(date: Date, holidays: readonly string[] = []): boolean {
  return !isWeekend(date) && !holidays.includes(isoDay(date));
}

/**
 * The date `n` business days after `date` (before it when `n` is negative).
 * `date` itself never counts. With n = 0 a copy of `date` is returned, even on a weekend.
 * The argument is not modified.
 */
export function addBusinessDays(date: Date, n: number, holidays: readonly string[] = []): Date {
  const result = new Date(date.getTime());
  const step = n < 0 ? -1 : 1;
  let moved = 0;
  while (moved < Math.abs(n)) {
    result.setUTCDate(result.getUTCDate() + step);
    if (isBusinessDay(result, holidays)) {
      moved++;
    }
  }
  return result;
}

/**
 * How many business days d satisfy from < d <= to. When `to` is before `from` the
 * result is the negative of the count for to < d <= from.
 */
export function businessDaysBetween(from: Date, to: Date, holidays: readonly string[] = []): number {
  if (to.getTime() < from.getTime()) {
    return -businessDaysBetween(to, from, holidays);
  }
  let count = 0;
  const cursor = new Date(from.getTime());
  while (cursor.getTime() < to.getTime()) {
    cursor.setUTCDate(cursor.getUTCDate() + 1);
    if (isBusinessDay(cursor, holidays)) {
      count++;
    }
  }
  return count;
}
