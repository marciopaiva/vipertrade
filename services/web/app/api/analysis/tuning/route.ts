import { NextResponse } from 'next/server';

// Proxy to ai-analyst POST /analyze/tuning (Format A): the deterministic Rust sweep
// grid + per-token perf + substitution, optionally narrated by OpenRouter. On-demand
// (triggered by a button), never on page load. The free-tier LLM can be slow (a 60k
// corpus narration has needed ~170s), so the timeout is generous; the deterministic
// grid always returns even if narration times out server-side.
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

export async function POST(req: Request) {
  let body: unknown = {};
  try {
    body = await req.json();
  } catch {
    body = {};
  }

  const baseUrls = uniqueBaseUrls(ANALYST_BASE_URLS);
  const errors: Array<{ baseUrl: string; message: string }> = [];

  for (const baseUrl of baseUrls) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 190_000);
    try {
      const response = await fetch(`${baseUrl}/analyze/tuning`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body ?? {}),
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
      const message = error instanceof Error ? error.message : String(error);
      errors.push({ baseUrl, message });
    } finally {
      clearTimeout(timeout);
    }
  }

  return NextResponse.json(
    {
      error: 'tuning_unavailable',
      message: 'could not reach ai-analyst /analyze/tuning',
      details: errors,
    },
    { status: 502 },
  );
}
