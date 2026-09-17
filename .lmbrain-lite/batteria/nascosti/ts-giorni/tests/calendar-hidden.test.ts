import { test } from "node:test";
import assert from "node:assert/strict";
import { addBusinessDays, businessDaysBetween, day, isoDay, isWeekend } from "../src/calendar.ts";

test("hidden: weekend days", () => {
  assert.equal(isWeekend(day("2026-09-19")), true);
  assert.equal(isWeekend(day("2026-09-20")), true);
  assert.equal(isWeekend(day("2026-09-21")), false);
});

test("hidden: zero returns a copy, even on a weekend", () => {
  const sat = day("2026-09-19");
  const got = addBusinessDays(sat, 0);
  assert.equal(isoDay(got), "2026-09-19");
  assert.notEqual(got, sat);
});

test("hidden: longer spans and the argument is untouched", () => {
  const start = day("2026-09-17");
  assert.equal(isoDay(addBusinessDays(start, 10)), "2026-10-01");
  assert.equal(isoDay(addBusinessDays(start, -10)), "2026-09-03");
  assert.equal(isoDay(addBusinessDays(day("2026-09-19"), 1)), "2026-09-21");
  assert.equal(isoDay(addBusinessDays(day("2026-09-20"), -1)), "2026-09-18");
  assert.equal(isoDay(start), "2026-09-17");
});

test("hidden: holidays backwards and across months", () => {
  const hol = ["2026-12-25", "2026-12-24", "2027-01-01"];
  assert.equal(isoDay(addBusinessDays(day("2026-12-28"), -2, hol)), "2026-12-22");
  assert.equal(isoDay(addBusinessDays(day("2026-12-23"), 3, hol)), "2026-12-30");
  assert.equal(isoDay(addBusinessDays(day("2026-12-31"), 1, hol)), "2027-01-04");
});

test("hidden: counting between dates", () => {
  assert.equal(businessDaysBetween(day("2026-09-21"), day("2026-09-14")), -5);
  assert.equal(businessDaysBetween(day("2026-09-19"), day("2026-09-20")), 0);
  assert.equal(businessDaysBetween(day("2026-09-18"), day("2026-09-18")), 0);
  assert.equal(businessDaysBetween(day("2026-09-17"), day("2026-09-22"), ["2026-09-21"]), 2);
});
