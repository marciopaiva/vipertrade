import { NextResponse } from 'next/server';

// Proxy to the ai-analyst service's heuristic descriptive analysis
// (`/analyze/recent`). This is deterministic, LLM-free diagnostics — evidence,
// expectancy, close-reason attribution, and advisory recommendations. There is
// no auto-tuning / apply path: tuning is done manually via the deterministic
// /sweep engine + the operator.
const ANALYST_BASE_URLS = [
  process.env.AI_ANALYST_URL,
  'http://ai-analyst:8087',
  'http://host.containers.internal:8087',
  'http://host.docker.internal:8087',
  'http://localhost:8087',
].filter(Boolean) as string[];

function uniqueBaseUrls(baseUrls: string[]): string[] {
  return Array.from(new Set(baseUrls.map(v => v.replace(/\/+$/, ''))));
}

export async function GET(req: Request) {
  const url = new URL(req.url);
  const hoursParam = Number(url.searchParams.get('hours'));
  const hours =
    Number.isFinite(hoursParam) && hoursParam > 0
      ? Math.min(hoursParam, 24 * 14)
      : Number(process.env.AI_ANALYST_LOOKBACK_HOURS || '24');

  const baseUrls = uniqueBaseUrls(ANALYST_BASE_URLS);
  const errors: Array<{ baseUrl: string; message: string }> = [];

  for (const baseUrl of baseUrls) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 8000);
    try {
      const response = await fetch(`${baseUrl}/analyze/recent?hours=${hours}`, {
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
      const message = error instanceof Error ? error.message : String(error);
      errors.push({ baseUrl, message });
    } finally {
      clearTimeout(timeout);
    }
  }

  return NextResponse.json(
    {
      error: 'analysis_unavailable',
      message: 'could not reach ai-analyst /analyze/recent',
      details: errors,
    },
    { status: 502 }
  );
}
