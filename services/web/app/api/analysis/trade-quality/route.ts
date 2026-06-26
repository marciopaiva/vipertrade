import { NextResponse } from 'next/server';

export const dynamic = 'force-dynamic';
export const revalidate = 0;
export const fetchCache = 'force-no-store';

// Proxy to the api service's live trade-quality metrics (realized closed trades,
// NOT a backtest): net/win, entry follow-through, trailing peak-capture, close-reason
// attribution, worst symbols. Feeds the /analysis "Ao Vivo" tab.
const API_BASE_URLS = [
  process.env.BACKEND_API_URL,
  'http://api:8080/api/v1',
  'http://vipertrade-api:8080/api/v1',
  'http://host.containers.internal:8080/api/v1',
  'http://host.docker.internal:8080/api/v1',
  process.env.NEXT_PUBLIC_API_URL ? `${process.env.NEXT_PUBLIC_API_URL}/api/v1` : null,
  'http://localhost:8080/api/v1',
].filter(Boolean) as string[];

function uniqueBaseUrls(baseUrls: string[]): string[] {
  return Array.from(new Set(baseUrls.map(v => v.replace(/\/+$/, ''))));
}

export async function GET(req: Request) {
  const url = new URL(req.url);
  const daysParam = Number(url.searchParams.get('days'));
  const days = Number.isFinite(daysParam) && daysParam > 0 ? Math.min(daysParam, 365) : 7;

  const baseUrls = uniqueBaseUrls(API_BASE_URLS);
  const errors: Array<{ baseUrl: string; message: string }> = [];

  for (const baseUrl of baseUrls) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 8000);
    try {
      const response = await fetch(`${baseUrl}/performance/trade-quality?days=${days}`, {
        cache: 'no-store',
        signal: controller.signal,
      });
      const raw = await response.text();
      const parsed = raw ? JSON.parse(raw) : null;
      if (!response.ok) {
        errors.push({ baseUrl, message: `http=${response.status} body=${raw || '<empty>'}` });
        continue;
      }
      return NextResponse.json(parsed, { status: 200 });
    } catch (error) {
      errors.push({ baseUrl, message: error instanceof Error ? error.message : String(error) });
    } finally {
      clearTimeout(timeout);
    }
  }

  return NextResponse.json(
    { error: 'trade_quality_unavailable', message: 'could not reach api /performance/trade-quality', details: errors },
    { status: 502 },
  );
}
