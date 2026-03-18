export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="min-h-screen bg-viper-navy">
      {/* Header */}
      <header className="border-b border-slate-700/50 bg-viper-navy/90 backdrop-blur-sm sticky top-0 z-50">
        <div className="container mx-auto px-4 py-4">
          <div className="flex items-center justify-between">
            <h1 className="text-xl font-bold text-viper-cyan">ViperTrade</h1>
            <nav className="flex items-center gap-4">
              <a href="/" className="text-sm text-slate-400 hover:text-viper-cyan">Dashboard</a>
              <a href="/trades" className="text-sm text-slate-400 hover:text-viper-cyan">Trades</a>
              <a href="/positions" className="text-sm text-slate-400 hover:text-viper-cyan">Positions</a>
              <a href="/settings" className="text-sm text-slate-400 hover:text-viper-cyan">Settings</a>
            </nav>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="container mx-auto px-4 py-6">
        {children}
      </main>
    </div>
  );
}
