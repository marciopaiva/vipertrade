/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: ['class'],
  content: ['./app/**/*.{ts,tsx}', './components/**/*.{ts,tsx}'],
  prefix: '',
  theme: {
    container: {
      center: true,
      padding: '2rem',
      screens: {
        '2xl': '1400px',
      },
    },
    extend: {
      // ViperTrade Brand Colors
      colors: {
        // shadcn/ui (CSS variables)
        border: 'hsl(var(--border))',
        input: 'hsl(var(--input))',
        ring: 'hsl(var(--ring))',
        background: 'hsl(var(--background))',
        foreground: 'hsl(var(--foreground))',
        primary: {
          DEFAULT: 'hsl(var(--primary))',
          foreground: 'hsl(var(--primary-foreground))',
        },
        secondary: {
          DEFAULT: 'hsl(var(--secondary))',
          foreground: 'hsl(var(--secondary-foreground))',
        },
        destructive: {
          DEFAULT: 'hsl(var(--destructive))',
          foreground: 'hsl(var(--destructive-foreground))',
        },
        muted: {
          DEFAULT: 'hsl(var(--muted))',
          foreground: 'hsl(var(--muted-foreground))',
        },
        accent: {
          DEFAULT: 'hsl(var(--accent))',
          foreground: 'hsl(var(--accent-foreground))',
        },
        popover: {
          DEFAULT: 'hsl(var(--popover))',
          foreground: 'hsl(var(--popover-foreground))',
        },
        card: {
          DEFAULT: 'hsl(var(--card))',
          foreground: 'hsl(var(--card-foreground))',
        },
        warn: 'hsl(var(--warn))',
        decision: 'hsl(var(--decision))',
      },

      // Dois degraus abaixo de `text-xs` (12px), que a escala padrão não tem e
      // os rótulos do HUD precisavam. Sem eles, cada tela inventava o seu
      // `text-[10px]` / `text-[11px]` — 47 usos no total.
      //
      // Em rem, não px, e isso é o ponto: o modo cockpit encolhe o `font-size`
      // da raiz (16px → 14px), então tamanho em rem acompanha a densidade e
      // tamanho em px não. Os usos arbitrários ignoravam o toggle.
      fontSize: {
        '3xs': ['0.625rem', { lineHeight: '0.875rem' }], // 10px @ raiz 16
        '2xs': ['0.6875rem', { lineHeight: '1rem' }], //    11px @ raiz 16
      },

      // Custom Fonts (loaded via next/font, wired through CSS variables)
      fontFamily: {
        display: ['var(--font-display)', 'Space Grotesk', 'sans-serif'], // Headings / UI
        mono: ['var(--font-mono)', 'JetBrains Mono', 'monospace'], // Trading numbers
        sans: ['var(--font-sans)', 'Inter', 'system-ui', 'sans-serif'], // Body
      },

      // Glows derivados dos tokens — acompanham o tema em vez de fixar o hex
      // antigo da paleta `viper.*`. `glow-purple` saiu junto: nada usava.
      boxShadow: {
        glow: '0 0 20px hsl(var(--primary) / 0.3)',
        'glow-lg': '0 0 30px hsl(var(--primary) / 0.5)',
        'glow-xl': '0 0 40px hsl(var(--primary) / 0.6)',
        'glow-accent': '0 0 20px hsl(var(--accent) / 0.3)',
      },

      borderRadius: {
        lg: 'var(--radius)',
        md: 'calc(var(--radius) - 2px)',
        sm: 'calc(var(--radius) - 4px)',
      },
      keyframes: {
        'accordion-down': {
          from: { height: '0' },
          to: { height: 'var(--radix-accordion-content-height)' },
        },
        'accordion-up': {
          from: { height: 'var(--radix-accordion-content-height)' },
          to: { height: '0' },
        },
        // Custom animations for trading
        'pulse-glow': {
          '0%, 100%': { opacity: '0.5', transform: 'scale(1)' },
          '50%': { opacity: '1', transform: 'scale(1.05)' },
        },
        float: {
          '0%, 100%': { transform: 'translateY(0)' },
          '50%': { transform: 'translateY(-5px)' },
        },
      },
      animation: {
        'accordion-down': 'accordion-down 0.2s ease-out',
        'accordion-up': 'accordion-up 0.2s ease-out',
        'pulse-glow': 'pulse-glow 2s ease-in-out infinite',
        float: 'float 3s ease-in-out infinite',
      },
    },
  },
  plugins: [require('tailwindcss-animate')],
};
