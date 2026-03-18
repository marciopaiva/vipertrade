'use client';

export default function Error({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-viper-navy">
      <div className="text-center space-y-6 p-8">
        {/* Error Icon */}
        <div className="text-6xl">⚠️</div>
        
        {/* Error Message */}
        <h2 className="text-2xl font-bold text-viper-red">Something went wrong!</h2>
        
        {/* Error Details (dev only) */}
        {process.env.NODE_ENV === 'development' && (
          <pre className="text-sm text-slate-400 bg-slate-800/50 p-4 rounded-lg max-w-md">
            {error.message}
          </pre>
        )}
        
        {/* Action Buttons */}
        <div className="flex gap-4 justify-center">
          <button
            onClick={reset}
            className="px-6 py-2 bg-viper-cyan text-viper-navy font-semibold rounded-lg hover:bg-cyan-400 transition-colors"
          >
            Try again
          </button>
          
          <a
            href="/"
            className="px-6 py-2 bg-slate-700 text-slate-200 font-semibold rounded-lg hover:bg-slate-600 transition-colors"
          >
            Go to Dashboard
          </a>
        </div>
      </div>
    </div>
  );
}
