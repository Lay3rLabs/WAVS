/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // Dark backgrounds - progressively lighter
        'charcoal-darkest': '#151413',
        'charcoal-dark': '#1E1C1B',
        'charcoal-medium': '#262423',
        'charcoal-light': '#37332E',

        // Warm text colors - progressively lighter
        'tan-muted': '#C5B5A3',
        'tan-warm': '#DBD1B5',
        'beige-warm': '#E8DDD0',
        'beige-light': '#EBE1C6',
        'cream-warm': '#EAE6DC',
        'cream-light': '#F5F1E7',

        // Neutral extremes
        'near-black': '#11131A',
        'whiteish': '#FAFAFA',

        // Reds
        'red-1': '#991b1b',
        'red-2': '#b91c1c',
        'red-3': '#dc2626',
        'red-4': '#ef4444',

        // Purples
        'purple-1': '#7e22ce',
        'purple-2': '#9333ea',
        'purple-3': '#a855f7',
      },
      fontFamily: {
        'sans': ['"Noto Sans"', 'sans-serif'],
      },
      keyframes: {
        'glow-green': {
          '0%, 100%': { boxShadow: '0 0 3px 1px rgba(34,197,94,0.2)' },
          '50%':       { boxShadow: '0 0 7px 2px rgba(34,197,94,0.45)' },
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
      },
      animation: {
        'glow-green':  'glow-green  2.5s ease-in-out infinite',
        'glow-amber':  'glow-amber  2.5s ease-in-out infinite',
        'glow-red':    'glow-red    2.5s ease-in-out infinite',
        'glow-yellow': 'glow-yellow 2.5s ease-in-out infinite',
      },
    },
  },
  plugins: [],
}
