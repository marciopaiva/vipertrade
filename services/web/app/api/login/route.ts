import { NextResponse } from 'next/server';
import { cookies } from 'next/headers';

export async function POST(request: Request) {
  try {
    const { username, password } = await request.json();

    // Validação simples (local-only)
    if (username === 'viperadmin' && password === '1234') {
      const response = NextResponse.json({ ok: true, message: 'Logged in' });

      // Set cookie (HttpOnly, SameSite Lax)
       const isSecure = process.env.VIPERTRADE_SECURE_COOKIE === 'true';
       response.cookies.set('vipertrade_session', 'viperadmin', {
         httpOnly: true,
         sameSite: 'lax',
         path: '/',
         secure: isSecure,
         maxAge: 60 * 60 * 24 * 30, // 30 days
       });

      return response;
    }

    return NextResponse.json({ error: 'Invalid credentials' }, { status: 401 });
  } catch {
    return NextResponse.json({ error: 'Bad request' }, { status: 400 });
  }
}
