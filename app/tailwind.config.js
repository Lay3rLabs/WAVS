/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // ── Legacy palette (existing app surfaces — keep working) ────
        'charcoal-darkest': '#1E1E1E',
        'charcoal-dark': '#222020',
        'charcoal-medium': '#2D2A2A',
        'charcoal-light': '#383232',

        'tan-muted': '#A89F96',
        'tan-warm': '#B9AFA4',
        'beige-warm': '#CEC3B7',
        'beige-light': '#DDD2C6',
        'cream-warm': '#DDD2C6',
        'cream-light': '#F2EAE2',

        'near-black': '#11131A',
        'whiteish': '#FAFAFA',

        'red-1': '#5B3A42',
        'red-2': '#814B56',
        'red-3': '#A7656F',
        'red-4': '#C38D99',

        'purple-1': '#4A345D',
        'purple-2': '#62497B',
        'purple-3': '#8265A1',
        'primary-600': '#9D7DC5',
        'primary-500': '#B49ADC',

        'success-900': '#255E52',
        'success-800': '#2F7B69',
        'success-700': '#3E9C81',
        'success-600': '#52B79D',
        'success-500': '#73D4BB',

        // ── Design system tokens (CSS-var driven, theme-swappable) ───
        ink: {
          canvas:           'var(--color-canvas)',
          bg:               'var(--color-bg)',
          surface:          'var(--color-surface)',
          'surface-raised': 'var(--color-surface-raised)',
          'surface-overlay':'var(--color-surface-overlay)',
          'surface-sunken': 'var(--color-surface-sunken)',

          border:           'var(--color-border)',
          'border-strong':  'var(--color-border-strong)',
          'border-focus':   'var(--color-border-focus)',

          fg:               'var(--color-fg)',
          'fg-secondary':   'var(--color-fg-secondary)',
          'fg-muted':       'var(--color-fg-muted)',
          'fg-faint':       'var(--color-fg-faint)',
          'fg-inverse':     'var(--color-fg-inverse)',

          accent:           'var(--color-accent)',
          'accent-hover':   'var(--color-accent-hover)',
          'accent-pressed': 'var(--color-accent-pressed)',
          'accent-fg':      'var(--color-accent-fg)',
          'accent-tint':    'var(--color-accent-tint)',
          'accent-edge':    'var(--color-accent-edge)',

          success:          'var(--color-success)',
          'success-tint':   'var(--color-success-tint)',
          'success-edge':   'var(--color-success-edge)',

          warning:          'var(--color-warning)',
          'warning-tint':   'var(--color-warning-tint)',
          'warning-edge':   'var(--color-warning-edge)',

          danger:           'var(--color-danger)',
          'danger-tint':    'var(--color-danger-tint)',
          'danger-edge':    'var(--color-danger-edge)',

          info:             'var(--color-info)',
          'info-tint':      'var(--color-info-tint)',
          'info-edge':      'var(--color-info-edge)',
        },
      },
      fontFamily: {
        // Legacy default — Montserrat for existing pages
        'sans': ['"Montserrat"', 'sans-serif'],
        // Design system
        'plex': ['"IBM Plex Sans"', 'system-ui', 'sans-serif'],
        'mono': ['"IBM Plex Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'monospace'],
        'serif': ['"IBM Plex Serif"', 'Georgia', 'serif'],
      },
      fontSize: {
        // Tighter, denser scale
        'xs':   ['11px', { lineHeight: '16px', letterSpacing: '0.02em' }],
        'sm':   ['12px', { lineHeight: '18px' }],
        'base': ['13px', { lineHeight: '20px' }],
        'md':   ['14px', { lineHeight: '22px' }],
        'lg':   ['16px', { lineHeight: '24px' }],
        'xl':   ['20px', { lineHeight: '28px', letterSpacing: '-0.01em' }],
        '2xl':  ['28px', { lineHeight: '34px', letterSpacing: '-0.02em' }],
        '3xl':  ['40px', { lineHeight: '46px', letterSpacing: '-0.025em' }],
        '4xl':  ['56px', { lineHeight: '60px', letterSpacing: '-0.03em' }],
      },
      borderRadius: {
        // Legacy
        'button': '99999px',
        'card-lg': '34px',
        'card-sm': '15px',
        // Design system
        'ds-none': 'var(--radius-none)',
        'ds-xs':   'var(--radius-xs)',
        'ds-sm':   'var(--radius-sm)',
        'ds-md':   'var(--radius-md)',
        'ds-lg':   'var(--radius-lg)',
        'ds-pill': 'var(--radius-pill)',
      },
      transitionTimingFunction: {
        'ds':       'var(--ease-out)',
        'ds-inout': 'var(--ease-in-out)',
      },
      transitionDuration: {
        'ds-instant': 'var(--dur-instant)',
        'ds-fast':    'var(--dur-fast)',
        'ds-base':    'var(--dur-base)',
        'ds-slow':    'var(--dur-slow)',
      },
      keyframes: {
        'glow-green': {
          '0%, 100%': { boxShadow: '0 0 3px 1px rgba(82,183,157,0.2)' },
          '50%':       { boxShadow: '0 0 7px 2px rgba(82,183,157,0.45)' },
        },
        'glow-amber': {
          '0%, 100%': { boxShadow: '0 0 3px 1px rgba(245,158,11,0.2)' },
          '50%':       { boxShadow: '0 0 7px 2px rgba(245,158,11,0.45)' },
        },
        'glow-red': {
          '0%, 100%': { boxShadow: '0 0 3px 1px rgba(239,68,68,0.2)' },
          '50%':       { boxShadow: '0 0 7px 2px rgba(239,68,68,0.45)' },
        },
        'glow-yellow': {
          '0%, 100%': { boxShadow: '0 0 3px 1px rgba(234,179,8,0.2)' },
          '50%':       { boxShadow: '0 0 7px 2px rgba(234,179,8,0.45)' },
        },
        'glow-primary': {
          '0%, 100%': { boxShadow: '0 0 3px 1px rgba(157,125,197,0.2)' },
          '50%':       { boxShadow: '0 0 7px 2px rgba(157,125,197,0.45)' },
        },
        'pulse-dot': {
          '0%, 100%': { opacity: '1', transform: 'scale(1)' },
          '50%':      { opacity: '0.55', transform: 'scale(0.92)' },
        },
        'shimmer': {
          '0%':   { backgroundPosition: '-200% 0' },
          '100%': { backgroundPosition: '200% 0' },
        },
        'toast-in': {
          '0%':   { opacity: '0', transform: 'translateY(8px) scale(0.98)' },
          '100%': { opacity: '1', transform: 'translateY(0) scale(1)' },
        },
        'toast-out': {
          '0%':   { opacity: '1', transform: 'translateY(0) scale(1)' },
          '100%': { opacity: '0', transform: 'translateX(40px) scale(0.96)' },
        },
      },
      animation: {
        'glow-green':   'glow-green   2.5s ease-in-out infinite',
        'glow-amber':   'glow-amber   2.5s ease-in-out infinite',
        'glow-red':     'glow-red     2.5s ease-in-out infinite',
        'glow-yellow':  'glow-yellow  2.5s ease-in-out infinite',
        'glow-primary': 'glow-primary 2.5s ease-in-out infinite',
        'pulse-dot':    'pulse-dot    1.6s ease-in-out infinite',
        'shimmer':      'shimmer      2.4s linear infinite',
        'toast-in':     'toast-in  180ms cubic-bezier(0.16, 1, 0.3, 1) both',
        'toast-out':    'toast-out 200ms cubic-bezier(0.65, 0, 0.35, 1) forwards',
      },
    },
  },
  plugins: [],
}
