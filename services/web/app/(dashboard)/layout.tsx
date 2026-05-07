import Link from 'next/link';
import { auth } from '@/app/api/auth/[...nextauth]/route';
import { redirect } from 'next/navigation';

export default async function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const session = await auth();
  if (!session) {
    redirect('/login');
  }

  return (
    <div className="min-h-screen bg-viper-navy">
      {/* Header */}
      <header className="border-b border-slate-700/50 bg-viper-navy/90 backdrop-blur-sm sticky top-0 z-50">
        <div className="container mx-auto px-4 py-4">
          <div className="flex items-center justify-between">
            <h1 className="text-xl font-bold text-viper-cyan">ViperTrade</h1>
            <nav className="flex items-center gap-4">
              <Link href="/" className="text-sm text-slate-400 hover:text-viper-cyan">
                Dashboard
              </Link>
              <Link href="/analysis" className="text-sm text-slate-400 hover:text-viper-cyan">
                Analysis
              </Link>
              <Link href="/trades" className="text-sm text-slate-400 hover:text-viper-cyan">
                Trades
              </Link>
              <Link href="/positions" className="text-sm text-slate-400 hover:text-viper-cyan">
                Positions
              </Link>
              <Link href="/settings" className="text-sm text-slate-400 hover:text-viper-cyan">
                Settings
              </Link>
            </nav>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="container mx-auto px-4 py-6">{children}</main>
    </div>
  );
}
