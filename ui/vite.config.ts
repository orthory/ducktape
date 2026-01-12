import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/documents': 'http://localhost:21922',
      '/d': 'http://localhost:21922',
      '/git': 'http://localhost:21922',
      '/ws': {
        target: 'ws://localhost:21922',
        ws: true,
      },
    },
  },
})
