import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

type CvInfo = { name: string; error: string | null }

// Placeholder: Task 12 builds the real UI.
export default function App() {
  const [cvs, setCvs] = useState<CvInfo[]>([])
  const [error, setError] = useState<string>()
  useEffect(() => {
    invoke<CvInfo[]>('list_cvs').then(setCvs, (e) => setError(JSON.stringify(e)))
  }, [])
  return (
    <main>
      <h1>jsoncv</h1>
      {error && <p role="alert">{error}</p>}
      <ul>{cvs.map((cv) => <li key={cv.name}>{cv.name}</li>)}</ul>
    </main>
  )
}
