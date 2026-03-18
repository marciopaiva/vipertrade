export default function DashboardLoading() {
  return (
    <div className="space-y-6 animate-pulse">
      {/* Architecture Flow Skeleton */}
      <div className="bg-slate-800/50 rounded-lg border border-slate-700/50 p-6">
        <div className="h-6 bg-slate-700 rounded w-1/4 mb-4" />
        <div className="h-64 bg-slate-700/50 rounded" />
      </div>

      {/* Wallet Overview Skeleton */}
      <div className="bg-slate-800/50 rounded-lg border border-slate-700/50 p-6">
        <div className="h-6 bg-slate-700 rounded w-1/3 mb-4" />
        <div className="grid grid-cols-3 gap-4">
          {[1, 2, 3].map((i) => (
            <div key={i} className="h-20 bg-slate-700/50 rounded" />
          ))}
        </div>
      </div>

      {/* Decision Matrix Skeleton */}
      <div className="bg-slate-800/50 rounded-lg border border-slate-700/50 p-6">
        <div className="h-6 bg-slate-700 rounded w-1/4 mb-4" />
        <div className="grid grid-cols-4 gap-4">
          {[1, 2, 3, 4].map((i) => (
            <div key={i} className="h-32 bg-slate-700/50 rounded" />
          ))}
        </div>
      </div>

      {/* Open Positions Skeleton */}
      <div className="bg-slate-800/50 rounded-lg border border-slate-700/50 p-6">
        <div className="h-6 bg-slate-700 rounded w-1/4 mb-4" />
        <div className="space-y-3">
          {[1, 2].map((i) => (
            <div key={i} className="h-24 bg-slate-700/50 rounded" />
          ))}
        </div>
      </div>

      {/* Closed Trades Skeleton */}
      <div className="bg-slate-800/50 rounded-lg border border-slate-700/50 p-6">
        <div className="h-6 bg-slate-700 rounded w-1/4 mb-4" />
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
            <div key={i} className="h-20 bg-slate-700/50 rounded" />
          ))}
        </div>
      </div>
    </div>
  );
}
