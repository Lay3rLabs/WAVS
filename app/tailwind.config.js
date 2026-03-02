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
        'charcoal-darkest': '#1E1E1E',
        'charcoal-dark': '#222020',
        'charcoal-medium': '#2D2A2A',
        'charcoal-light': '#383232',

        // Warm text colors - progressively lighter
        'tan-muted': '#A89F96',
        'tan-warm': '#B9AFA4',
        'beige-warm': '#CEC3B7',
        'beige-light': '#DDD2C6',
        'cream-warm': '#DDD2C6',
        'cream-light': '#F2EAE2',

        // Neutral extremes
        'near-black': '#11131A',
        'whiteish': '#FAFAFA',

        // Reds (alert)
        'red-1': '#5B3A42',
        'red-2': '#814B56',
        'red-3': '#A7656F',
        'red-4': '#C38D99',

        // Purples (primary brand)
        'purple-1': '#4A345D',
        'purple-2': '#62497B',
        'purple-3': '#8265A1',
        'primary-600': '#9D7DC5',
        'primary-500': '#B49ADC',

        // Success greens
        'success-900': '#255E52',
        'success-800': '#2F7B69',
        'success-700': '#3E9C81',
        'success-600': '#52B79D',
        'success-500': '#73D4BB',
      },
      fontFamily: {
        'sans': ['"Montserrat"', 'sans-serif'],
      },
      borderRadius: {
        'button': '99999px',
        'card-lg': '34px',
        'card-sm': '15px',
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
      },
      animation: {
        'glow-green':   'glow-green   2.5s ease-in-out infinite',
        'glow-amber':   'glow-amber   2.5s ease-in-out infinite',
        'glow-red':     'glow-red     2.5s ease-in-out infinite',
        'glow-yellow':  'glow-yellow  2.5s ease-in-out infinite',
        'glow-primary': 'glow-primary 2.5s ease-in-out infinite',
      },
    },
  },
  plugins: [],
}
