export default function Loading() {
  return (
    <div className="flex items-center justify-center min-h-screen bg-viper-navy">
      <div className="flex flex-col items-center gap-4">
        {/* Spinner */}
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-viper-cyan" />
        
        {/* Loading Text */}
        <p className="text-viper-cyan text-sm animate-pulse">Loading ViperTrade...</p>
      </div>
    </div>
  );
}
