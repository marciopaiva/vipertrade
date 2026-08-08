import { NextResponse } from 'next/server';

export const dynamic = 'force-dynamic';
export const revalidate = 0;
export const fetchCache = 'force-no-store';

// Proxy para o snapshot do checklist de swing que alimenta a matriz de decisão.
//
// Precisa de handler próprio: o rewrite genérico de `next.config` aponta para
// `http://api:8080/api/:path*`, sem o `/v1`, então `/api/strategy/swing-matrix`
// caía em 404 na API e a matriz renderizava vazia com "BTC bloqueia" — que é o
// estado padrão quando não há dado, e portanto indistinguível de um filtro
// macro realmente fechado.
const API_BASE_URLS = [
  process.env.BACKEND_API_URL,
  'http://api:8080/api/v1',
  'http://vipertrade-api:8080/api/v1',
  'http://host.containers.internal:8080/api/v1',
  'http://host.docker.internal:8080/api/v1',
  process.env.NEXT_PUBLIC_API_URL
    ? `${process.env.NEXT_PUBLIC_API_URL}/api/v1`
    : null,
  'http://localhost:8080/api/v1',
].filter(Boolean) as string[];

function uniqueBaseUrls(baseUrls: string[]): string[] {
  return Array.from(new Set(baseUrls.map(v => v.replace(/\/+$/, ''))));
}

export async function GET() {
  const baseUrls = uniqueBaseUrls(API_BASE_URLS);
  const errors: Array<{ baseUrl: string; message: string }> = [];

  for (const baseUrl of baseUrls) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 8000);
    try {
      const response = await fetch(`${baseUrl}/strategy/swing-matrix`, {
        cache: 'no-store',
        signal: controller.signal,
      });
      const raw = await response.text();
      const parsed = raw ? JSON.parse(raw) : null;
      if (!response.ok) {
        errors.push({
          baseUrl,
          message: `http=${response.status} body=${raw || '<empty>'}`,
        });
        continue;
      }
      return NextResponse.json(parsed, { status: 200 });
    } catch (error) {
      errors.push({
        baseUrl,
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      clearTimeout(timeout);
    }
  }

  return NextResponse.json(
    {
      error: 'swing_matrix_unavailable',
      message: 'could not reach api /strategy/swing-matrix',
      details: errors,
    },
    { status: 502 }
  );
}
