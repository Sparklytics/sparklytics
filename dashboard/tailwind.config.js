/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // Brand
        spark: {
          50:  '#E8FDF5',
          100: '#D1FBEB',
          400: '#00E696',
          500: '#00D084',  // Primary
          600: '#00B86D',
          700: '#009955',
          DEFAULT: '#00D084',
          dim: '#00B86D',
          subtle: 'rgba(0, 208, 132, 0.08)',
        },
        // Semantic
        up:      '#10B981',
        down:    '#EF4444',
        warn:    '#F59E0B',
        neutral: '#3B82F6',
        // Chart palette
        'chart-0': '#00D084',
        'chart-1': '#3B82F6',
        'chart-2': '#F59E0B',
        'chart-3': '#EC4899',
        'chart-4': '#8B5CF6',
        'chart-5': '#06B6D4',
      },
      backgroundColor: {
        canvas:  'var(--canvas)',
        's1':    'var(--surface-1)',
        's2':    'var(--surface-2)',
        'input': 'var(--surface-input)',
      },
      textColor: {
        ink:     'var(--ink)',
        'ink-2': 'var(--ink-2)',
        'ink-3': 'var(--ink-3)',
        'ink-4': 'var(--ink-4)',
      },
      borderColor: {
        line:     'var(--line)',
        'line-2': 'var(--line-2)',
        'line-3': 'var(--line-3)',
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', 'sans-serif'],
        mono: ['IBM Plex Mono', 'Menlo', 'monospace'],
      },
      fontSize: {
        'h1':      ['32px', { lineHeight: '40px', letterSpacing: '-0.5px', fontWeight: '600' }],
        'h2':      ['24px', { lineHeight: '32px', letterSpacing: '-0.5px', fontWeight: '600' }],
        'h3':      ['18px', { lineHeight: '28px', letterSpacing: '-0.5px', fontWeight: '600' }],
        'h4':      ['14px', { lineHeight: '22px', letterSpacing: '0px',    fontWeight: '600' }],
        'body':    ['14px', { lineHeight: '22px', letterSpacing: '0px',    fontWeight: '400' }],
        'sm':      ['12px', { lineHeight: '18px', letterSpacing: '0px',    fontWeight: '400' }],
        'code':    ['12px', { lineHeight: '22px', letterSpacing: '0px',    fontWeight: '500' }],
        'code-lg': ['13px', { lineHeight: '20px', letterSpacing: '0px',    fontWeight: '400' }],
      },
    },
  },
  plugins: [],
}
