import './globals.css';
import { Space_Grotesk, JetBrains_Mono, Inter } from 'next/font/google';

// Single source of truth for typography (resolves the prior font drift:
// tailwind declared Inter/JetBrains while globals.css applied Space Grotesk).
const display = Space_Grotesk({
  subsets: ['latin'],
  variable: '--font-display',
  display: 'swap',
});
const mono = JetBrains_Mono({
  subsets: ['latin'],
  variable: '--font-mono',
  display: 'swap',
});
const sans = Inter({
  subsets: ['latin'],
  variable: '--font-sans',
  display: 'swap',
});

export const metadata = {
  title: 'ViperTrade Dashboard',
  description: 'Operational dashboard for ViperTrade backend services',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html
      lang="en"
      className={`${display.variable} ${mono.variable} ${sans.variable}`}
    >
      <head>
        {/* Apply the saved density before paint so there's no flash and no
            hydration mismatch (runs outside React). */}
        <script
          dangerouslySetInnerHTML={{
            __html: `try{var d=localStorage.getItem('viper-density');if(d==='cockpit')document.documentElement.dataset.density=d;}catch(e){}`,
          }}
        />
      </head>
      <body>{children}</body>
    </html>
  );
}
