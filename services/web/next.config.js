/** @type {import('next').NextConfig} */
const nextConfig = {
  // Output standalone for Docker multi-stage
  output: 'standalone',
  
  // React strict mode (helps find bugs)
  reactStrictMode: true,
  
  // Environment variables
  env: {
    NEXT_PUBLIC_API_URL: process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080',
    NEXT_PUBLIC_WS_URL: process.env.NEXT_PUBLIC_WS_URL || 'ws://localhost:8080/ws',
    NEXT_PUBLIC_TRADING_MODE: process.env.NEXT_PUBLIC_TRADING_MODE || 'paper',
  },
  
  // Rewrites for API backend
  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: 'http://vipertrade-api:8080/api/:path*',
      },
    ];
  },
  
  // Security headers
  async headers() {
    return [
      {
        source: '/(.*)',
        headers: [
          { key: 'X-Content-Type-Options', value: 'nosniff' },
          { key: 'X-Frame-Options', value: 'DENY' },
          { key: 'X-XSS-Protection', value: '1; mode=block' },
          { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
          { key: 'Strict-Transport-Security', value: 'max-age=31536000; includeSubDomains' },
        ],
      },
    ];
  },
  
  // Image optimization
  images: {
    formats: ['image/avif', 'image/webp'],
    deviceSizes: [640, 750, 828, 1080, 1200, 1920, 2048, 3840],
    imageSizes: [16, 32, 48, 64, 96, 128, 256, 384],
  },
  
  // Compression
  compress: true,
  
  // Source maps in development only
  productionBrowserSourceMaps: process.env.NODE_ENV === 'development',
  
  // Turbopack config (empty for default behavior)
  turbopack: {},
}

module.exports = nextConfig
