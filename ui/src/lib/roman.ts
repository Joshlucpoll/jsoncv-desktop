const NUMERALS: [number, string][] = [
  [1000, 'M'], [900, 'CM'], [500, 'D'], [400, 'CD'], [100, 'C'], [90, 'XC'],
  [50, 'L'], [40, 'XL'], [10, 'X'], [9, 'IX'], [5, 'V'], [4, 'IV'], [1, 'I'],
]

/** Roman numeral for a positive integer, for decorative counts only; anything else comes back as Arabic. */
export function roman(n: number): string {
  if (!Number.isInteger(n) || n < 1 || n > 3999) return String(n)
  let out = ''
  for (const [v, s] of NUMERALS) for (; n >= v; n -= v) out += s
  return out
}
