# carrello

Cart totals in integer cents. The rules, in the order they are applied:

1. **Line gross** = unit price × quantity.
2. **Buy X get Y** (`buyXgetY`, per SKU): for every complete group of X + Y units, Y units are
   free. 7 units with buy 2 get 1 free → two complete groups → 2 free units.
3. **Percent** (`percent`): applies to one SKU, or to every line when `sku` is missing, on what
   is left after step 2. Several percent discounts on one line apply one after the other.
   Each step is rounded half-up to the cent.
4. **VAT** is computed per line on the discounted amount (the line's `net`), with the rate
   of the line's category, rounded half-up to the cent. An unknown category is an error.
5. **Fixed** (`fixed`) discounts are subtracted from the total after VAT; the total never
   goes below zero, and `discount` reports what was actually subtracted.

Code: `money.ts` (rounding), `discounts.ts` (steps 2–3), `tax.ts` (step 4), `cart.ts`
(everything together).
