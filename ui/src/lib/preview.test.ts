import { describe, expect, it } from 'vitest'
import { pageCount, placePage, withHead } from './preview.ts'

describe('withHead', () => {
  it('puts the CSP first in <head>', () => {
    const out = withHead('<html><head><title>x</title></head><body></body></html>')
    expect(out.indexOf('Content-Security-Policy')).toBeLessThan(out.indexOf('<title>'))
    expect(out).toContain('<base target="_blank">')
  })
  it('prepends when there is no <head>, and is not fooled by <header>', () => {
    expect(withHead('<p>hi</p>').startsWith('<meta')).toBe(true)
    expect(withHead('<body><header>x</header></body>').startsWith('<meta')).toBe(true)
  })
})

describe('pageCount', () => {
  it('counts started pages, ignoring rounding', () => {
    expect(pageCount(1123, 1123)).toBe(1)
    expect(pageCount(1125, 1123)).toBe(1)
    expect(pageCount(1200, 1123)).toBe(2)
    expect(pageCount(0, 1123)).toBe(1)
  })
})

describe('placePage', () => {
  it('centres a page that fits', () => expect(placePage(-300, 1000, 500)).toBe(250))
  it('lets a larger page follow the wanted position', () => expect(placePage(-200, 1000, 1500)).toBe(-200))
  it('stops a larger page at its edges', () => {
    expect(placePage(50, 1000, 1500)).toBe(0)
    expect(placePage(-900, 1000, 1500)).toBe(-500)
  })
  it('meets the centred position as the page grows past the view, so nothing jumps', () => expect(placePage(-80, 1000, 1000)).toBe(0))
})
