import { discountedLine, fixedTotal, type Discount, type LineInput } from "./discounts.ts";
import { vatFor } from "./tax.ts";

export interface CartLine extends LineInput {
  category: string;
}

export interface ReceiptLine {
  sku: string;
  gross: number;
  net: number;
  vat: number;
}

export interface Receipt {
  lines: ReceiptLine[];
  /** Sum of the lines' net. */
  subtotal: number;
  /** Sum of the lines' VAT. */
  vat: number;
  /** Fixed discount actually subtracted. */
  discount: number;
  total: number;
}

export function checkout(lines: readonly CartLine[], discounts: readonly Discount[] = []): Receipt {
  const receiptLines: ReceiptLine[] = lines.map((line) => {
    const gross = line.unitCents * line.quantity;
    const net = discountedLine(line, discounts);
    const vat = vatFor(gross, line.category);
    return { sku: line.sku, gross, net, vat };
  });
  const subtotal = receiptLines.reduce((sum, l) => sum + l.net, 0);
  const vat = receiptLines.reduce((sum, l) => sum + l.vat, 0);
  const beforeFixed = subtotal + vat;
  const discount = Math.min(fixedTotal(discounts), beforeFixed);
  return { lines: receiptLines, subtotal, vat, discount, total: beforeFixed - discount };
}
