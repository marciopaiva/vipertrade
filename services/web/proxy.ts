import { NextResponse } from 'next/server';
import type { NextRequest } from 'next/server';

// Auth is opt-in. Running locally we keep it off (the default) so the
// console is directly accessible; set WEB_AUTH_ENABLED=true to enforce the
// session-cookie gate. Proper auth (hashed creds + signed token) is tracked
// in #32 — the current cookie check is presence-only and not a real guard.
const AUTH_ENABLED = process.env.WEB_AUTH_ENABLED === 'true';

const PUBLIC_PATHS = [
  '/login',
  '/api/login',
  '/api/logout',
  '/_next',
  '/favicon.ico',
  '/api/health',
  '/logo.png',
  '/icon.png',
  '/apple-icon.png',
];

export function proxy(request: NextRequest) {
  if (!AUTH_ENABLED) {
    return NextResponse.next();
  }

  const { pathname } = request.nextUrl;
  if (PUBLIC_PATHS.some((p) => pathname.startsWith(p))) {
    return NextResponse.next();
  }

  if (!request.cookies.get('vipertrade_session')) {
    return NextResponse.redirect(new URL('/login', request.url));
  }

  return NextResponse.next();
}

export const config = {
  matcher: '/:path*',
};
