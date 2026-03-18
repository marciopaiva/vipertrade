// services/web/app/error.tsx
'use client';

import { useEffect } from 'react';

export default function Error({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    console.error('Application error:', error);
  }, [error]);

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-[#0a1929]">
      <div className="text-center space-y-4">
        <h2 className="text-2xl font-bold text-red-400">Something went wrong!</h2>
        <p className="text-cyan-300 text-sm">{error.message}</p>
        <button
          onClick={reset}
          className="px-6 py-2 bg-gradient-to-r from-cyan-500 to-green-500 text-black font-semibold rounded-lg hover:opacity-90 transition"
        >
          Try again
        </button>
      </div>
    </div>
  );
}
