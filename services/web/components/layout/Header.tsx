export default function Header() {
  return (
    <header className="border-b border-slate-700/50 bg-viper-navy/90 backdrop-blur-sm sticky top-0 z-50">
      <div className="container mx-auto px-4 py-4">
        <div className="flex items-center justify-between">
          {/* Logo */}
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-viper-cyan to-viper-green flex items-center justify-center">
              <span className="text-viper-navy font-bold text-xl">V</span>
            </div>
            <div>
              <h1 className="text-xl font-bold text-viper-cyan">ViperTrade</h1>
              <p className="text-xs text-slate-400">Lead Trader Bot</p>
            </div>
          </div>

          {/* Navigation */}
          <nav className="flex items-center gap-6">
            <a href="/" className="text-sm text-slate-400 hover:text-viper-cyan transition-colors">
              Dashboard
            </a>
            <a href="/trades" className="text-sm text-slate-400 hover:text-viper-cyan transition-colors">
              Trades
            </a>
            <a href="/positions" className="text-sm text-slate-400 hover:text-viper-cyan transition-colors">
              Positions
            </a>
            <a href="/settings" className="text-sm text-slate-400 hover:text-viper-cyan transition-colors">
              Settings
            </a>
          </nav>

          {/* Status Indicator */}
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-viper-green animate-pulse" />
            <span className="text-xs text-slate-400">Live</span>
          </div>
        </div>
      </div>
    </header>
  );
}
