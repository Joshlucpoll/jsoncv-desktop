import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App.tsx'
import './styles.css'
import { MAC } from './lib/platform.ts'

document.documentElement.classList.toggle('mac', MAC)

if (import.meta.env.DEV && new URLSearchParams(location.search).has('mock')) await import('./dev/mockTauri.ts')

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
