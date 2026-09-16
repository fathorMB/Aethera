import { test } from "node:test";
import assert from "node:assert/strict";
import { checkout } from "../src/cart.ts";
import { freeUnits } from "../src/discounts.ts";
import { vatFor } from "../src/tax.ts";

test("hidden: free units", () => {
  assert.equal(freeUnits(7, 2, 1), 2);
  assert.equal(freeUnits(4, 3, 1), 1);
  assert.equal(freeUnits(3, 3, 1), 0);
  assert.equal(freeUnits(11, 3, 2), 4);
  assert.equal(freeUnits(0, 1, 1), 0);
});

test("hidden: buyXgetY then percent then vat", () => {
  const r = checkout([{ sku: "tea", unitCents: 100, quantity: 7, category: "food" }], [
    { kind: "percent", sku: "tea", percent: 10 },
    { kind: "buyXgetY", sku: "tea", buy: 2, free: 1 },
  ]);
  assert.deepEqual(r.lines, [{ sku: "tea", gross: 700, net: 450, vat: 45 }]);
  assert.equal(r.total, 495);
});

test("hidden: stacked percents and sku filter", () => {
  const r = checkout(
    [
      { sku: "a", unitCents: 999, quantity: 1, category: "standard" },
      { sku: "b", unitCents: 1000, quantity: 1, category: "books" },
    ],
    [
      { kind: "percent", percent: 10 },
      { kind: "percent", sku: "a", percent: 10 },
    ],
  );
  assert.deepEqual(r.lines, [
    { sku: "a", gross: 999, net: 809, vat: 178 },
    { sku: "b", gross: 1000, net: 900, vat: 36 },
  ]);
  assert.equal(r.subtotal, 1709);
  assert.equal(r.vat, 214);
  assert.equal(r.total, 1923);
});

test("hidden: vat rounding and unknown categories", () => {
  assert.equal(vatFor(25, "standard"), 6);
  assert.equal(vatFor(0, "food"), 0);
  assert.throws(() => vatFor(100, "garden"));
  assert.throws(() => checkout([{ sku: "x", unitCents: 0, quantity: 0, category: "" }]));
});

test("hidden: fixed discounts after vat, capped at the total", () => {
  const line = { sku: "mug", unitCents: 1000, quantity: 1, category: "standard" };
  const r1 = checkout([line], [{ kind: "fixed", cents: 100 }, { kind: "fixed", cents: 50 }]);
  assert.equal(r1.vat, 220);
  assert.equal(r1.discount, 150);
  assert.equal(r1.total, 1070);
  const r2 = checkout([line], [{ kind: "fixed", cents: 5000 }]);
  assert.equal(r2.discount, 1220);
  assert.equal(r2.total, 0);
  const r3 = checkout([]);
  assert.deepEqual(r3, { lines: [], subtotal: 0, vat: 0, discount: 0, total: 0 });
});
