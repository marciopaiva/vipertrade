import { NextResponse } from 'next/server';
import type { NextRequest } from 'next/server';

export function middleware(request: NextRequest) {
  const pathname = request.nextUrl.pathname;

  // Log para debug
  console.log(`[Middleware] path=${pathname} cookies=${request.cookies.get('vipertrade_session')?.value || 'none'}`);

   // Rotas públicas
   const publicPaths = [
     '/login',
     '/api/login',
     '/api/logout',
     '/_next',
     '/favicon.ico',
     '/api/health',
     // Arquivos estáticos da raiz
     '/logo.png',
     '/icon.png',
     '/apple-icon.png',
   ];
  if (publicPaths.some(p => pathname.startsWith(p))) {
    return NextResponse.next();
  }

  // Verificar sessão via cookie customizado
  const session = request.cookies.get('vipertrade_session');
  if (!session) {
    console.log(`[Middleware] no session — redirecting to /login`);
    const loginUrl = new URL('/login', request.url);
    return NextResponse.redirect(loginUrl);
  }

  console.log(`[Middleware] session found, allowing`);
  return NextResponse.next();
}

export const config = {
  matcher: '/:path*',
};


