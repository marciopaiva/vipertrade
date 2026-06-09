'use client';

export default function DashboardError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return (
    <div className="flex flex-col items-center justify-center min-h-[400px] bg-secondary/50 rounded-lg border border-border/50 p-8">
      <div className="text-center space-y-4">
        {/* Error Icon */}
        <div className="text-4xl">❌</div>

        {/* Error Message */}
        <h3 className="text-xl font-bold text-viper-red">Dashboard Error</h3>

        {/* Error Details (dev only) */}
        {process.env.NODE_ENV === 'development' && (
          <pre className="text-xs text-muted-foreground bg-card/50 p-3 rounded max-w-sm">
            {error.message}
          </pre>
        )}

        {/* Action Buttons */}
        <div className="flex gap-3 justify-center">
          <button
            onClick={reset}
            className="px-4 py-2 bg-viper-cyan text-viper-navy font-semibold rounded hover:bg-primary transition-colors"
          >
            Retry
          </button>

          <a
            href="/api/health"
            target="_blank"
            className="px-4 py-2 bg-secondary text-foreground font-semibold rounded hover:bg-secondary transition-colors"
          >
            Check API
          </a>
        </div>
      </div>
    </div>
  );
}
