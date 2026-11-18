import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

process.env.TZ = 'UTC' // deterministic weekday/date labels in tests

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
})
