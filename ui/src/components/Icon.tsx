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
  laurel: 'M7 13.8C4.3 13 2.7 10.4 3 7M9 13.8c2.7-.8 4.3-3.4 4-6.8M3 7c-.6-.9-.5-2 .2-2.8.6.9.5 2-.2 2.8M13 7c.6-.9.5-2-.2-2.8-.6.9-.5 2 .2 2.8M3.3 9.8c-1-.1-1.8-.8-2-1.8 1 .1 1.8.8 2 1.8M12.7 9.8c1-.1 1.8-.8 2-1.8-1 .1-1.8.8-2 1.8M5 12c-1 .1-1.9-.4-2.4-1.3 1-.1 1.9.4 2.4 1.3M11 12c1 .1 1.9-.4 2.4-1.3-1-.1-1.9.4-2.4 1.3',
  // CV sections
  basics: 'M8 8a2.5 2.5 0 100-5 2.5 2.5 0 000 5zM3 13.5c.6-2.3 2.6-3.5 5-3.5s4.4 1.2 5 3.5',
  work: 'M2.5 5.5h11v7h-11zM6 5.5V4a1 1 0 011-1h2a1 1 0 011 1v1.5M2.5 9h11',
  volunteer: 'M8 13s-5-3-5-6.5A2.5 2.5 0 018 5a2.5 2.5 0 015 1.5C13 10 8 13 8 13z',
  education: 'M1.5 6L8 3l6.5 3L8 9 1.5 6zM4 7.3v3.2c1.2 1 2.5 1.5 4 1.5s2.8-.5 4-1.5V7.3M14.5 6v3.5',
  awards: 'M5 2.5h6v3a3 3 0 01-6 0v-3zM5 4H3a2 2 0 002 2.5M11 4h2a2 2 0 01-2 2.5M8 8.5v3M5.5 13.5h5M6.5 11.5h3',
  certificates: 'M8 9.5a3.5 3.5 0 100-7 3.5 3.5 0 000 7zM5.8 8.7L5 14l3-1.5 3 1.5-.8-5.3',
  publications: 'M2.5 3.5h4a1.5 1.5 0 011.5 1.5v8.5a1.5 1.5 0 00-1.5-1.5h-4V3.5zM13.5 3.5h-4A1.5 1.5 0 008 5v8.5a1.5 1.5 0 011.5-1.5h4V3.5z',
  skills: 'M9 1.5L3.5 9H8l-1 5.5L12.5 7H8l1-5.5z',
  languages: 'M8 14A6 6 0 108 2a6 6 0 000 12zM2 8h12M8 2c1.6 1.7 2.4 3.7 2.4 6S9.6 12.3 8 14M8 2C6.4 3.7 5.6 5.7 5.6 8s.8 4.3 2.4 6',
  interests: 'M8 2l1.8 3.8 4.2.5-3.1 2.9.8 4.1L8 11.3l-3.7 2 .8-4.1L2 6.3l4.2-.5L8 2z',
  references: 'M3 7.5h3.5V11H3V7.5zm0 0C3 5.5 4 4.3 6 4M9.5 7.5H13V11H9.5V7.5zm0 0c0-2 1-3.2 3-3.5',
  projects: 'M8 2l6 3-6 3-6-3 6-3zM2 8l6 3 6-3M2 11l6 3 6-3',
  sideProjects: 'M6 2h4M6.5 2v4L3 12.5A1 1 0 003.9 14h8.2a1 1 0 00.9-1.5L9.5 6V2M4.5 10h7',
  // UI
  braces: 'M5.5 3C4 3 4 4 4 5.5S3 8 2.5 8C3 8 4 8.5 4 10.5S4 13 5.5 13M10.5 3C12 3 12 4 12 5.5S13 8 13.5 8c-.5 0-1.5.5-1.5 2.5S12 13 10.5 13',
  sliders: 'M2 4.5h7M12 4.5h2M2 11.5h2M7 11.5h7M10.5 3v3M5.5 10v3',
  copy: 'M5.5 5.5v-2a1 1 0 011-1h6a1 1 0 011 1v6a1 1 0 01-1 1h-2M3.5 5.5h6a1 1 0 011 1v6a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1z',
  folder: 'M2 4.5a1 1 0 011-1h3l1.5 1.5H13a1 1 0 011 1V12a1 1 0 01-1 1H3a1 1 0 01-1-1V4.5z',
  palette: 'M8 14a6 6 0 116-6c0 1.5-1.2 2-2.2 2H10a1.2 1.2 0 00-.8 2c.4.5.1 2-1.2 2zM5 7.5h.01M7 5h.01M10 5.2h.01',
  plug: 'M6 2v3M10 2v3M4.5 5h7v2.5a3.5 3.5 0 01-7 0V5zM8 11v3',
  pages: 'M4 1.5h6l3 3v4M4 1.5a1 1 0 00-1 1v4M3 9.5v4a1 1 0 001 1h8a1 1 0 001-1v-4',
  pencil: 'M10.5 2.5l3 3-8 8H2.5v-3l8-8zM9 4l3 3',
  download: 'M8 2.5v8M4.5 7L8 10.5 11.5 7M2.5 13.5h11',
  scroll: 'M4 1.5h8a1 1 0 011 1v11a1 1 0 01-1 1H4a1 1 0 01-1-1v-11a1 1 0 011-1zM5.5 5h5M5.5 8h5M5.5 11h3',
} as const

export type IconName = keyof typeof PATHS

export function Icon({ name, size = 14 }: { name: IconName; size?: number }) {
  return (
    <svg className="icon" width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={PATHS[name]} />
    </svg>
  )
}
