import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export type CvInfo = { name: string; theme: string | null; tailored_from: string | null; modified_ms: number; error: string | null }
export type CvDoc = { name: string; cv: unknown; base: string }
export type Saved = { commit: string; base: string }
export type Version = { commit: string; author: string; message: string; time_ms: number }
export type ThemeInfo = { id: string; builtin: boolean; meta: { name: string; description: string; page: { size: 'A4' | 'Letter' } } }
export type ThemeReport = { render_error: { message: string; line: number | null } | null; missing: string[] }
export type SchemaError = { path: string; message: string }
export type ApiError = { kind: string; message: string; errors?: SchemaError[] }
export type ExportFormat = 'pdf' | 'html' | 'json'
export type LibraryConfig = { library: string; inSyncFolder: boolean }

/** Dev-only: browser run with `?mock` serves a static page instead of the preview scheme. */
export const MOCK = import.meta.env.DEV && new URLSearchParams(location.search).has('mock')

export const DEFAULT_THEME = 'builtin:classic'

export const errorMessage = (e: unknown): string =>
  typeof e === 'object' && e && 'message' in e ? String((e as ApiError).message) : String(e)

export const listCvs = () => invoke<CvInfo[]>('list_cvs')
export const readCv = (name: string) => invoke<CvDoc>('read_cv', { name })
export const saveDraft = (name: string, baseCv: unknown, draft: unknown) => invoke<Saved>('save_draft', { name, baseCv, draft })
export const createTailored = (slug: string, from: string) => invoke<CvInfo>('create_tailored', { slug, from })
export const renameCv = (name: string, newSlug: string) => invoke<CvInfo>('rename_cv', { name, newSlug })
export const deleteCv = (name: string) => invoke<void>('delete_cv', { name })
export const history = (name: string, limit: number) => invoke<Version[]>('history', { name, limit })
export const diff = (name: string, commit: string) => invoke<string>('diff', { name, commit })
export const restore = (name: string, commit: string) => invoke<Saved>('restore', { name, commit })
export const listThemes = () => invoke<ThemeInfo[]>('list_themes')
export const themeReport = (id: string) => invoke<ThemeReport>('theme_report', { id })
export const duplicateTheme = (id: string, newId: string) => invoke<ThemeInfo>('duplicate_theme', { id, newId })
export const setCvTheme = (name: string, themeId: string) => invoke<Saved>('set_cv_theme', { name, themeId })
export const exportCv = (name: string, format: ExportFormat) => invoke<string>('export', { name, format })
export const printCv = (name: string) => invoke<void>('print_cv', { name })
export const addAssetFromPath = (path: string) => invoke<string>('add_asset_from_path', { path })
export const getConfig = () => invoke<LibraryConfig>('get_config')
export const setLibrary = (path: string) => invoke<LibraryConfig>('set_library', { path })
export const reveal = (path: string) => invoke<void>('reveal', { path })
export const binaryPath = () => invoke<string>('binary_path')

/** Returns the unlisten function. */
export const onLibraryChanged = async (cb: (paths: string[]) => void) =>
  listen<{ paths: string[] }>('library-changed', (e) => cb(e.payload.paths))

export function previewUrl(cv: string, theme?: string, commit?: string): string {
  if (MOCK) return '/dev-preview.html'
  const q = new URLSearchParams({ cv })
  if (theme) q.set('theme', theme)
  if (commit) q.set('commit', commit)
  return /windows/i.test(navigator.userAgent)
    ? `http://jsoncv-preview.localhost/render?${q}`
    : `jsoncv-preview://localhost/render?${q}`
}
export const getSchema = () => invoke<Record<string, unknown>>('get_schema')
