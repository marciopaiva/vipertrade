import './globals.css';
import { SessionProvider } from 'next-auth/react';

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
    <html lang="en">
      <body>
        <SessionProvider>{children}</SessionProvider>
      </body>
    </html>
  );
}
