import React from 'react'

// Cloud/self-hosted mode flag — set VITE_MODE=cloud at build time for cloud mode
export const IS_CLOUD = import.meta.env.VITE_MODE === 'cloud'
export const CLERK_PUBLISHABLE_KEY = import.meta.env.VITE_CLERK_PUBLISHABLE_KEY ?? ''

function App() {
  return (
    <div
      className="min-h-screen flex items-center justify-center"
      style={{ background: 'var(--canvas)', color: 'var(--ink)' }}
    >
      <div className="text-center space-y-4">
        <div className="flex items-center justify-center gap-3">
          {/* Lightning bolt logo mark */}
          <svg
            width="28"
            height="32"
            viewBox="0 0 28 32"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            aria-hidden="true"
          >
            <polyline
              points="18,2 8,18 14,18 10,30 22,12 16,12 20,2"
              stroke="#00D084"
              strokeWidth="2.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              fill="none"
            />
          </svg>
          <h1
            className="font-mono font-semibold"
            style={{ fontSize: '24px', color: 'var(--ink)' }}
          >
            Sparklytics
          </h1>
        </div>

        <p style={{ color: 'var(--ink-2)', fontSize: '14px' }}>
          Developer-first analytics. Zero config. Full control.
        </p>

        <p style={{ color: 'var(--ink-3)', fontSize: '12px' }}>
          Work in progress — Sprint 0 scaffolding
        </p>

        <div
          className="inline-block px-3 py-1 rounded font-mono text-xs"
          style={{
            background: 'var(--spark-subtle)',
            color: 'var(--spark)',
            border: '1px solid rgba(0, 208, 132, 0.2)',
          }}
        >
          Dashboard coming in Sprint 2
        </div>
      </div>
    </div>
  )
}

export default App
