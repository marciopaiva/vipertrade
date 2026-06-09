// Empty by default => relative URLs (e.g. /api/v1/...), so client-side fetches
// stay same-origin and are proxied to the backend by the Next rewrite. An
// absolute base would cause CORS in dev (:3000) and be unreachable in-cluster
// (the browser can't resolve `api:8080`). Matches useDashboard.
const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || '';

export const endpoints = {
  // Dashboard
  dashboard: '/api/dashboard',

  // Market Data
  marketSignals: '/api/v1/market-signals',

  // Strategy decisions (cockpit): latest decision per symbol + consensus indicators
  decisions: '/api/v1/decisions',

  // Positions
  positions: '/api/v1/positions',

  // Trades
  trades: '/api/v1/trades',
  tradesToday: '/api/v1/trades/today',

  // Wallet
  wallet: '/api/v1/wallet',

  // Services
  services: '/api/v1/services',

  // Analytics
  analytics: '/api/v1/analytics',

  // Health
  health: '/api/health',
};

export async function fetchApi<T>(endpoint: string): Promise<T> {
  const url = `${API_BASE_URL}${endpoint}`;
  const res = await fetch(url, { cache: 'no-store' });

  if (!res.ok) {
    throw new Error(`API error: ${res.status}`);
  }

  return res.json();
}
