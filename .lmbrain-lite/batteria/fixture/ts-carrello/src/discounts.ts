import { roundHalfUp } from "./money.ts";

export type Discount =
  | { kind: "buyXgetY"; sku: string; buy: number; free: number }
  | { kind: "percent"; sku?: string; percent: number }
  | { kind: "fixed"; cents: number };

export interface LineInput {
  sku: string;
  unitCents: number;
  quantity: number;
}

/** Units that are free for this line under one buyXgetY discount. */
export function freeUnits(quantity: number, buy: number, free: number): number {
  const groups = Math.round(quantity / (buy + free));
  return groups * free;
}

/** The line amount after buyXgetY and percent discounts (steps 2 and 3 of the README). */
export function discountedLine(line: LineInput, discounts: readonly Discount[]): number {
  let amount = line.unitCents * line.quantity;
  for (const d of discounts) {
    if (d.kind === "buyXgetY" && d.sku === line.sku) {
      amount -= freeUnits(line.quantity, d.buy, d.free) * line.unitCents;
    }
  }
  for (const d of discounts) {
    if (d.kind === "percent" && (d.sku === undefined || d.sku === line.sku)) {
      amount = roundHalfUp((amount * (100 - d.percent)) / 100);
    }
  }
  return amount;
}

/** Sum of the fixed discounts. */
export function fixedTotal(discounts: readonly Discount[]): number {
  let sum = 0;
  for (const d of discounts) {
    if (d.kind === "fixed") sum += d.cents;
  }
  return sum;
}
