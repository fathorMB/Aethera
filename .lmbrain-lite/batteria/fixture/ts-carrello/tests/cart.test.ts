import { test } from "node:test";
import assert from "node:assert/strict";
import { checkout } from "../src/cart.ts";
import { formatCents } from "../src/money.ts";

test("plain cart", () => {
  const r = checkout([
    { sku: "pasta", unitCents: 150, quantity: 2, category: "food" },
    { sku: "novel", unitCents: 1200, quantity: 1, category: "books" },
  ]);
  assert.equal(r.subtotal, 1500);
  assert.equal(r.vat, 30 + 48);
  assert.equal(formatCents(r.total), "15.78");
});

test("vat is computed on the discounted amount", () => {
  const r = checkout([{ sku: "mug", unitCents: 1000, quantity: 1, category: "standard" }], [
    { kind: "percent", percent: 50 },
  ]);
  assert.deepEqual(r.lines, [{ sku: "mug", gross: 1000, net: 500, vat: 110 }]);
  assert.equal(r.total, 610);
});

test("buy two get one free counts complete groups only", () => {
  const r = checkout([{ sku: "soap", unitCents: 100, quantity: 5, category: "standard" }], [
    { kind: "buyXgetY", sku: "soap", buy: 2, free: 1 },
  ]);
  assert.equal(r.lines[0].net, 400);
});

test("unknown category is an error", () => {
  assert.throws(() => checkout([{ sku: "x", unitCents: 1, quantity: 1, category: "toys" }]));
});
