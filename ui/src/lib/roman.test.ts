import { expect, test } from 'vitest'
import { roman } from './roman.ts'

test('roman numerals', () => {
  expect([1, 4, 9, 14, 40, 90, 400, 1994, 3999].map(roman)).toEqual(['I', 'IV', 'IX', 'XIV', 'XL', 'XC', 'CD', 'MCMXCIV', 'MMMCMXCIX'])
  expect([0, -1, 2.5, 4000].map(roman)).toEqual(['0', '-1', '2.5', '4000'])
})
