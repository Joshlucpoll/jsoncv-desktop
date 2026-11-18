// DEV ONLY: lets the UI run in a plain browser at /?mock for screenshots.
import { mockIPC } from '@tauri-apps/api/mocks'
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

mockIPC(
  (cmd, args) => {
    const a = args as Record<string, string>
    switch (cmd) {
      case 'list_cvs': return cvs
      case 'list_themes': return themes
      case 'get_config': return { library: '~/Documents/CVs', inSyncFolder: false }
      case 'set_cv_theme': {
        const cv = cvs.find((c) => c.name === a.name)
        if (cv) cv.theme = a.themeId
        return { commit: 'abc', base: 'def' }
      }
      default: return null
    }
  },
  { shouldMockEvents: true },
)
