export default function Footer() {
  return (
    <footer className="border-t border-slate-700/50 bg-viper-navy/90 mt-auto">
      <div className="container mx-auto px-4 py-6">
        <div className="flex items-center justify-between">
          {/* Copyright */}
          <div className="text-sm text-slate-400">
            © {new Date().getFullYear()} ViperTrade. All rights reserved.
          </div>

          {/* Links */}
          <div className="flex items-center gap-4 text-sm text-slate-400">
            <a href="#" className="hover:text-viper-cyan transition-colors">
              Documentation
            </a>
            <a href="#" className="hover:text-viper-cyan transition-colors">
              API
            </a>
            <a href="#" className="hover:text-viper-cyan transition-colors">
              Support
            </a>
          </div>

          {/* Version */}
          <div className="text-xs text-slate-500">
            v{process.env.NEXT_PUBLIC_VERSION || '0.1.0'}
          </div>
        </div>
      </div>
    </footer>
  );
}
