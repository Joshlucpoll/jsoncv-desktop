// DEV ONLY: lets the UI run in a plain browser at /?mock for screenshots.
import { emit } from '@tauri-apps/api/event'
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'
import schema from '../../../crates/core/schema/jsoncv.schema.json'
import sample from '../../../crates/core/resources/sample.cv.json'
import type { CvInfo, ThemeInfo } from '../api.ts'

const now = Date.now()
const cv = (name: string, folder: string | null, pin: number | null, over: Partial<CvInfo> = {}): CvInfo =>
  ({ name, folder, pin, theme: null, continuous: false, modified_ms: now, error: null, ...over })
const cvs: CvInfo[] = [
  cv('general', null, 0, { modified_ms: now + 1 }),
  cv('stripe', 'applications', null, { theme: 'builtin:two-column' }),
  cv('monzo', 'applications', 1, { continuous: true }),
  cv('broken-draft', null, null, { error: 'Invalid JSON at line 14' }),
]
const folders = ['applications', 'archive']
let order: string[] = []
const theme = (id: string, name: string, builtin: boolean): ThemeInfo => ({ id, builtin, meta: { name, description: '', page: { size: 'A4' } } })
const themes = [theme('builtin:classic', 'Classic', true), theme('builtin:two-column', 'Two-column', true), theme('my-theme', 'my-theme', false)]

// The CV dev-preview.html renders, so preview clicks map to real fields.
const sampleCv = sample

const M = 60_000
const ver = (commit: string, author: string, message: string, ago: number) => ({ commit, author, message, time_ms: now - ago })
const versions = [
  ver('v0', 'You', 'Edit CV', 1 * M), ver('v1', 'You', 'Edit CV', 2 * M), ver('v2', 'You', 'Edit CV', 4 * M),
  ver('v3', 'Agent', 'Tailor summary for Stripe', 40 * M),
  ver('v4', 'External', 'Changed outside the app', 3 * 3_600_000),
  ver('v5', 'You', 'Edit CV', 26 * 3_600_000), ver('v6', 'You', 'Edit CV', 26 * 3_600_000 + 2 * M),
  ver('v7', 'You', 'Create general', 20 * 86_400_000),
]

let cfg: Record<string, unknown> = { library: '~/Documents/CVs' }

mockIPC(
  (cmd, args) => {
    const a = args as Record<string, string>
    switch (cmd) {
      case 'list_cvs': return cvs.map((c) => ({ ...c })) // fresh objects, as the real IPC returns
      case 'list_folders': return [...folders]
      case 'get_order': return [...order]
      case 'set_order': order = [...(args as { keys: string[] }).keys]; return null
      case 'set_pin_order': (args as { names: string[] }).names.forEach((n, i) => { const c = cvs.find((x) => x.name === n); if (c) c.pin = i }); return null
      case 'set_pinned': {
        const c = cvs.find((x) => x.name === a.name)!
        const pinned = cvs.filter((x) => x.pin !== null && x !== c).sort((x, y) => x.pin! - y.pin!)
        if ((args as { pinned?: boolean }).pinned) pinned.push(c)
        cvs.forEach((x) => (x.pin = null))
        pinned.forEach((x, i) => (x.pin = i))
        return null
      }
      case 'move_cv': { const c = cvs.find((x) => x.name === a.name)!; c.folder = a.folder ?? null; return { ...c } }
      case 'duplicate_cv': { const f = cvs.find((x) => x.name === a.from)!; const c = { ...f, name: a.slug, pin: null }; cvs.push(c); return { ...c } }
      case 'create_cv': { const c = cv(a.slug, a.folder ?? null, null); cvs.push(c); return { ...c } }
      case 'rename_cv': { const c = cvs.find((x) => x.name === a.name)!; c.name = a.newSlug; return { ...c } }
      case 'delete_cv': cvs.splice(cvs.findIndex((x) => x.name === a.name), 1); return null
      case 'create_folder': folders.push(a.name); return null
      case 'rename_folder': folders[folders.indexOf(a.name)] = a.newName; cvs.forEach((x) => { if (x.folder === a.name) x.folder = a.newName }); return null
      case 'delete_folder': folders.splice(folders.indexOf(a.name), 1); return null
      case 'list_themes': return [...themes] // a fresh array, as the real command returns
      case 'get_config': return cfg
      case 'set_library': cfg = { ...cfg, library: a.path }; return cfg
      case 'set_page_mode': { const cv = cvs.find((c) => c.name === a.name); if (cv) cv.continuous = !!(args as { continuous?: boolean }).continuous; return { commit: 'pm', base: 'h4' } }
      case 'connect_agent': if (a.agent === 'codex') throw { kind: 'NotFound', message: 'not found: the `codex` command. Is Codex installed? Copy the setup prompt instead' }; return `Added jsoncv to ${a.agent}. Restart it to use it.`
      case 'theme_report': return a.id === 'my-theme'
        ? { render_error: null, missing: ['awards', 'work[].highlights', 'basics.location.region'] }
        : { render_error: null, missing: [] }
      case 'duplicate_theme': { const t = theme(a.newId, a.name ?? a.newId, false); themes.push(t); return t }
      case 'delete_theme': themes.splice(themes.findIndex((t) => t.id === a.id), 1); return null
      case 'export': return a.path
      case 'print_cv': case 'reveal': return null
      case 'list_editors': return [{ id: 'vscode', label: 'VS Code' }, { id: 'zed', label: 'Zed' }]
      case 'open_theme_in': console.info(`[mock] open ${a.id} in ${a.editor}`); return null
      case 'agent_has_edited': return false
      case 'open_link': console.info(`[mock] open ${a.url}`); return null
      case 'theme_guide_html': return '<h1>Writing a jsoncv theme</h1><p>A theme turns a CV (JSON) into one HTML page. See <a href="https://docs.rs/minijinja">MiniJinja</a>.</p><table><thead><tr><th>Helper</th><th>Use</th></tr></thead><tbody><tr><td><code>markdown</code></td><td>Render Markdown</td></tr></tbody></table><pre><code>{{ cv.basics.name }}</code></pre>'
      case 'theme_dir_path': return `~/Documents/CVs/themes/${a.id}`
      // Close/quit guard: log instead of closing so the flow is visible in the browser console.
      case 'exit_app': case 'plugin:window|destroy': console.info(`[mock] ${cmd}: app would close`); return null
      case 'library_stats': return { bytes: 2_480_000, assets: 3, history_bytes: 412_000 }
      case 'binary_path': return '/Applications/JsonCV.app/Contents/MacOS/jsoncv'
      case 'set_cv_theme': {
        const cv = cvs.find((c) => c.name === a.name)
        if (cv) cv.theme = a.themeId
        return { commit: 'abc', base: 'def' }
      }
      case 'get_schema': return schema
      case 'read_cv': return { name: a.name, cv: sampleCv, base: 'h1' }
      case 'history': return [...versions]
      case 'restore': { const commit = `r${versions.length}`; versions.unshift({ commit, author: 'You', message: `Restored version ${a.commit}`, time_ms: Date.now() }); return { commit, base: 'h3' } }
      case 'diff': return '--- a/general.cv.json\n+++ b/general.cv.json\n@@ -2,3 +2,3 @@\n   "basics": {\n-    "name": "Ada Lovelace",\n+    "name": "Ada King",\n     "label": "Mathematician"'
      case 'change_diff': return '--- empty\n+++ v3\n@@ -5,1 +5,1 @@\n-    "summary": "Engineer."\n+    "summary": "Engineer who ships payments infrastructure."'
      case 'preview_html': return fetch('/dev-preview.html').then((r) => r.text())
      case 'save_draft': return { commit: 'c1', base: 'h2' }
      case 'add_asset_from_path': return 'assets/photo.png'
      case 'plugin:dialog|save': return `/Users/me/Documents/${(a?.options as unknown as { defaultPath: string }).defaultPath}`
      case 'plugin:dialog|open': return a?.options && (a.options as unknown as { directory?: boolean }).directory ? '/Users/me/Library/Mobile Documents/CVs' : '/tmp/photo.png'
      default: return null
    }
  },
  { shouldMockEvents: true },
)
mockWindows('main') // getCurrentWindow() needs window metadata

// Simulates an agent edit over MCP: run __mockAgentEdit() in the console.
;(window as unknown as { __mockAgentEdit: () => void }).__mockAgentEdit = () => {
  versions.unshift(ver(`a${versions.length}`, 'Agent', 'Tightened summary', 0))
  void emit('library-changed', { paths: ['cvs/general.cv.json'] })
}
