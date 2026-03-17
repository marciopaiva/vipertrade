/** @type {import('next').NextConfig} */
const nextConfig = {
  // Output: standalone for Docker optimization
  output: 'standalone',
  
  // React strict mode (better dev experience)
  reactStrictMode: true,
  
  // SWC minifier (faster than Terser)
  swcMinify: true,
  
  // Environment variables
  env: {
    NEXT_PUBLIC_API_URL: process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080',
    NEXT_PUBLIC_WS_URL: process.env.NEXT_PUBLIC_WS_URL || 'ws://localhost:8080/ws',
  },
  
  // Disable Next.js telemetry
  poweredByHeader: false,
  
  // Compression
  compress: true,
  
  // Source maps in development only
  productionBrowserSourceMaps: process.env.NODE_ENV === 'development',
  
  // Image optimization
  images: {
    formats: ['image/avif', 'image/webp'],
    deviceSizes: [640, 750, 828, 1080, 1200, 1920, 2048, 3840],
    imageSizes: [16, 32, 48, 64, 96, 128, 256, 384],
    minimumCacheTTL: 60,
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
          { key: 'Permissions-Policy', value: 'camera=(), microphone=(), geolocation=()' },
          { key: 'X-Permitted-Cross-Domain-Policies', value: 'none' },
        ],
      },
    ];
  },
  
  // API rewrites (proxy)
  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: 'http://vipertrade-api:8080/api/:path*',
      },
    ];
  },
}

module.exports = nextConfig
