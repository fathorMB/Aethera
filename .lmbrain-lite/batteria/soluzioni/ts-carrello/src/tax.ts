import { roundHalfUp } from "./money.ts";

/** VAT rates in percent, by product category. */
export const VAT_RATES: Readonly<Record<string, number>> = {
  standard: 22,
  food: 10,
  books: 4,
};

export function vatRate(category: string): number {
  const rate = Object.hasOwn(VAT_RATES, category) ? VAT_RATES[category] : undefined;
  if (rate === undefined) {
    throw new Error(`unknown VAT category: ${category}`);
  }
  return rate;
}

/** VAT on `cents` for `category`, rounded half-up to the cent. */
export function vatFor(cents: number, category: string): number {
  return roundHalfUp((cents * vatRate(category)) / 100);
}
