import { roundHalfUp } from "./money.ts";

/** VAT rates in percent, by product category. */
export const VAT_RATES: Readonly<Record<string, number>> = {
  standard: 22,
  food: 10,
  books: 4,
};

export function vatRate(category: string): number {
  return VAT_RATES[category] ?? 0;
}

/** VAT on `cents` for `category`, rounded half-up to the cent. */
export function vatFor(cents: number, category: string): number {
  return roundHalfUp((cents * vatRate(category)) / 100);
}
