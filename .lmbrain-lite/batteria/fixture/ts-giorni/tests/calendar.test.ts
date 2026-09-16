import { test } from "node:test";
import assert from "node:assert/strict";
import { addBusinessDays, businessDaysBetween, day, isoDay } from "../src/calendar.ts";

test("friday plus one business day is monday", () => {
  assert.equal(isoDay(addBusinessDays(day("2026-09-18"), 1)), "2026-09-21");
});

test("negative counts go backwards", () => {
  assert.equal(isoDay(addBusinessDays(day("2026-09-21"), -1)), "2026-09-18");
});

test("holidays are skipped", () => {
  assert.equal(isoDay(addBusinessDays(day("2026-09-17"), 1, ["2026-09-18"])), "2026-09-21");
});

test("business days in a week", () => {
  assert.equal(businessDaysBetween(day("2026-09-14"), day("2026-09-21")), 5);
});
