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
    },
  },
  plugins: [],
}
