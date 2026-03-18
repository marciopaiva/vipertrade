import Link from 'next/link';

export default function NotFound() {
  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-viper-navy">
      <div className="text-center space-y-6 p-8">
        {/* 404 Text */}
        <h1 className="text-9xl font-bold text-viper-cyan opacity-20">404</h1>
        
        {/* Message */}
        <div className="space-y-2">
          <h2 className="text-2xl font-bold text-slate-200">Page Not Found</h2>
          <p className="text-slate-400">The page you're looking for doesn't exist.</p>
        </div>
        
        {/* Back Button */}
        <Link
          href="/"
          className="inline-block px-6 py-2 bg-viper-cyan text-viper-navy font-semibold rounded-lg hover:bg-cyan-400 transition-colors"
        >
          Go to Dashboard
        </Link>
      </div>
    </div>
  );
}
