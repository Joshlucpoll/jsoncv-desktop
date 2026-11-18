// DEV ONLY: lets the UI run in a plain browser at /?mock for screenshots.
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'
import schema from '../../../crates/core/schema/jsoncv.schema.json'
import type { CvInfo, ThemeInfo } from '../api.ts'

const now = Date.now()
const cvs: CvInfo[] = [
  { name: 'main', theme: null, tailored_from: null, modified_ms: now, error: null },
  { name: 'tailored/stripe', theme: 'builtin:two-column', tailored_from: 'main', modified_ms: now, error: null },
  { name: 'tailored/monzo', theme: null, tailored_from: 'main', modified_ms: now, error: null },
  { name: 'tailored/broken-draft', theme: null, tailored_from: 'main', modified_ms: now, error: 'Invalid JSON at line 14' },
]
const theme = (id: string, name: string, builtin: boolean): ThemeInfo => ({ id, builtin, meta: { name, description: '', page: { size: 'A4' } } })
const themes = [theme('builtin:classic', 'Classic', true), theme('builtin:two-column', 'Two-column', true), theme('my-theme', 'my-theme', false)]

const sampleCv = {
  basics: {
    name: 'Ada Lovelace', label: 'Mathematician', email: 'ada@example.com', image: 'assets/ada.jpg',
    location: { city: 'London', countryCode: 'GB' },
    profiles: [{ network: 'GitHub', username: 'ada', url: 'https://github.com/ada' }],
  },
  work: [{ name: 'Analytical Engines Ltd', position: 'Programmer', startDate: '1842-01', highlights: ['Wrote the first program'] }],
  skills: [{ name: 'Mathematics', keywords: ['Algebra', 'Calculus'] }],
}

const M = 60_000
const ver = (commit: string, author: string, message: string, ago: number) => ({ commit, author, message, time_ms: now - ago })
const versions = [
  ver('v0', 'You', 'Edit CV', 1 * M), ver('v1', 'You', 'Edit CV', 2 * M), ver('v2', 'You', 'Edit CV', 4 * M),
  ver('v3', 'Agent', 'Tailor summary for Stripe', 40 * M),
  ver('v4', 'External', 'Changed outside the app', 3 * 3_600_000),
  ver('v5', 'You', 'Edit CV', 26 * 3_600_000), ver('v6', 'You', 'Edit CV', 26 * 3_600_000 + 2 * M),
  ver('v7', 'You', 'Create main', 20 * 86_400_000),
]

let cfg: Record<string, unknown> = new URLSearchParams(location.search).has('sync')
  ? { library: '~/Library/Mobile Documents/com~apple~CloudDocs/CVs', inSyncFolder: true }
  : { library: '~/Documents/CVs', inSyncFolder: false }

mockIPC(
  (cmd, args) => {
    const a = args as Record<string, string>
    switch (cmd) {
      case 'list_cvs': return cvs
      case 'list_themes': return themes
      case 'get_config': return cfg
      case 'set_library': cfg = { library: a.path, inSyncFolder: /iCloud|Mobile Documents/.test(a.path) }; return cfg
      case 'theme_report': return a.id === 'my-theme'
        ? { render_error: null, missing: ['awards', 'work[].highlights', 'basics.location.region'] }
        : { render_error: null, missing: [] }
      case 'duplicate_theme': { const t = theme(a.newId, a.newId, false); themes.push(t); return t }
      case 'export': return `~/Documents/CVs/exports/${a.name.replace('/', '-')}-classic.${a.format}`
      case 'print_cv': case 'reveal': return null
      case 'theme_dir_path': return `~/Documents/CVs/themes/${a.id}`
      // Close/quit guard: log instead of closing so the flow is visible in the browser console.
      case 'exit_app': case 'plugin:window|destroy': console.info(`[mock] ${cmd}: app would close`); return null
      case 'binary_path': return '/Applications/JsonCV.app/Contents/MacOS/jsoncv'
      case 'set_cv_theme': {
        const cv = cvs.find((c) => c.name === a.name)
        if (cv) cv.theme = a.themeId
        return { commit: 'abc', base: 'def' }
      }
      case 'get_schema': return schema
      case 'read_cv': return { name: a.name, cv: sampleCv, base: 'h1' }
      case 'history': return [...versions]
      case 'restore': versions.unshift({ commit: `r${versions.length}`, author: 'You', message: 'Restore version', time_ms: Date.now() }); return { commit: 'r', base: 'h3' }
      case 'diff': return '--- a/main.cv.json\n+++ b/main.cv.json\n@@ -2,3 +2,3 @@\n   "basics": {\n-    "name": "Ada Lovelace",\n+    "name": "Ada King",\n     "label": "Mathematician"'
      case 'save_draft': return { commit: 'c1', base: 'h2' }
      case 'add_asset_from_path': return 'assets/photo.png'
      case 'plugin:dialog|open': return a?.options && (a.options as unknown as { directory?: boolean }).directory ? '/Users/me/Library/Mobile Documents/CVs' : '/tmp/photo.png'
      default: return null
    }
  },
  { shouldMockEvents: true },
)
mockWindows('main') // getCurrentWindow() needs window metadata
