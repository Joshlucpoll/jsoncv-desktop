// 16px stroke icons (Lucide-style paths, drawn inline so no icon package is needed).
const PATHS = {
  chevronRight: 'M6 4l4 4-4 4',
  chevronDown: 'M4 6l4 4 4-4',
  up: 'M8 13V3M4 7l4-4 4 4',
  down: 'M8 3v10M4 9l4 4 4-4',
  x: 'M4 4l8 8M12 4l-8 8',
  plus: 'M8 3v10M3 8h10',
  more: 'M3.5 8h.01M8 8h.01M12.5 8h.01',
  warn: 'M8 2.5l6 11H2l6-11zM8 6.5v3M8 11.5h.01',
  gear: 'M8 10a2 2 0 100-4 2 2 0 000 4zM13 8a5 5 0 00-.1-1l1.4-1.1-1.3-2.3-1.7.6a5 5 0 00-1.7-1L9.3 1.5H6.7l-.3 1.7a5 5 0 00-1.7 1l-1.7-.6-1.3 2.3L3.1 7a5 5 0 000 2l-1.4 1.1 1.3 2.3 1.7-.6a5 5 0 001.7 1l.3 1.7h2.6l.3-1.7a5 5 0 001.7-1l1.7.6 1.3-2.3L12.9 9c.1-.3.1-.7.1-1z',
  branch: 'M5 3v6a3 3 0 003 3h3M9 10l2 2-2 2',
  minus: 'M3 8h10',
  check: 'M3 8.5l3 3 7-7',
} as const

export type IconName = keyof typeof PATHS

export function Icon({ name, size = 14 }: { name: IconName; size?: number }) {
  return (
    <svg className="icon" width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={PATHS[name]} />
    </svg>
  )
}
