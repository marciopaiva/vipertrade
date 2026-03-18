/**
 * ViperTrade Web Configuration
 * 
 * Centralized configuration for the web application.
 * All feature flags and settings should be defined here.
 */

export const config = {
  // Application
  app: {
    name: 'ViperTrade',
    version: process.env.NEXT_PUBLIC_VERSION || '0.1.0',
    environment: process.env.NODE_ENV || 'development',
  },

  // API Configuration
  api: {
    baseUrl: process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080',
    wsUrl: process.env.NEXT_PUBLIC_WS_URL || 'ws://localhost:8080/ws',
    timeout: 30000, // 30 seconds
    retries: 3,
  },

  // Trading Configuration
  trading: {
    mode: (process.env.NEXT_PUBLIC_TRADING_MODE as 'paper' | 'testnet' | 'mainnet') || 'paper',
    profile: (process.env.NEXT_PUBLIC_TRADING_PROFILE as 'CONSERVATIVE' | 'MEDIUM' | 'AGGRESSIVE') || 'MEDIUM',
  },

  // Feature Flags
  features: {
    websocket: process.env.NEXT_PUBLIC_ENABLE_WEBSOCKET !== 'false',
    analytics: process.env.NEXT_PUBLIC_ENABLE_ANALYTICS !== 'false',
    livePositions: process.env.NEXT_PUBLIC_ENABLE_LIVE_POSITIONS !== 'false',
    notifications: process.env.NEXT_PUBLIC_ENABLE_NOTIFICATIONS !== 'false',
  },

  // UI Configuration
  ui: {
    refreshInterval: parseInt(process.env.NEXT_PUBLIC_REFRESH_INTERVAL || '5000', 10),
    tradesPerPage: parseInt(process.env.NEXT_PUBLIC_TRADES_PER_PAGE || '10', 10),
    darkMode: process.env.NEXT_PUBLIC_DARK_MODE !== 'false',
  },

  // Performance
  performance: {
    maxCachedItems: 100,
    debounceMs: 300,
    throttleMs: 1000,
  },
} as const;

export type Config = typeof config;
