import { NextResponse } from 'next/server';

// Market-sentiment card data. Two independent external sources, fetched
// server-side (avoids CORS, keeps responses cacheable, hides nothing secret):
//   - Crypto Fear & Greed Index — alternative.me (free, no key). The same
//     0–100 index Bybit surfaces; updates roughly once a day.
//   - Long/Short account ratio — Bybit public market endpoint (no auth).
// Each source degrades independently: if one fails we still return the other.

const FNG_URL = 'https://api.alternative.me/fng/?limit=1';
const BYBIT_RATIO_URL =
  'https://api.bybit.com/v5/market/account-ratio?category=linear&symbol=BTCUSDT&period=1d&limit=1';

// Both sources move slowly; cache at the Next data layer so frequent client
// polling collapses to at most one upstream call per window per instance.
// Next requires this segment config to be a statically analyzable literal.
export const revalidate = 300;
const REVALIDATE_SECONDS = 300;

interface FearGreed {
  value: number;
  classification: string;
  updatedAt: string | null;
}

interface LongShort {
  symbol: string;
  longPct: number;
  shortPct: number;
  updatedAt: string | null;
}

interface SentimentResponse {
  fearGreed: FearGreed | null;
  longShort: LongShort | null;
  errors: string[];
}

async function fetchJson(url: string): Promise<unknown> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 8000);
  try {
    const res = await fetch(url, {
      signal: controller.signal,
      next: { revalidate: REVALIDATE_SECONDS },
    });
    if (!res.ok) {
      throw new Error(`http=${res.status}`);
    }
    return await res.json();
  } finally {
    clearTimeout(timeout);
  }
}

function parseFearGreed(raw: unknown): FearGreed {
  const data = (raw as { data?: unknown[] })?.data?.[0] as
    | { value?: string; value_classification?: string; timestamp?: string }
    | undefined;
  const value = Number(data?.value);
  if (!Number.isFinite(value)) {
    throw new Error('fng: missing value');
  }
  const ts = Number(data?.timestamp);
  return {
    value,
    classification: data?.value_classification ?? 'Unknown',
    updatedAt: Number.isFinite(ts) ? new Date(ts * 1000).toISOString() : null,
  };
}

function parseLongShort(raw: unknown): LongShort {
  const row = (raw as { result?: { list?: unknown[] } })?.result?.list?.[0] as
    | { symbol?: string; buyRatio?: string; sellRatio?: string; timestamp?: string }
    | undefined;
  const buy = Number(row?.buyRatio);
  const sell = Number(row?.sellRatio);
  if (!Number.isFinite(buy) || !Number.isFinite(sell)) {
    throw new Error('bybit: missing ratio');
  }
  const ts = Number(row?.timestamp);
  return {
    symbol: row?.symbol ?? 'BTCUSDT',
    longPct: Math.round(buy * 1000) / 10,
    shortPct: Math.round(sell * 1000) / 10,
    updatedAt: Number.isFinite(ts) ? new Date(ts).toISOString() : null,
  };
}

export async function GET() {
  const errors: string[] = [];

  const [fngResult, ratioResult] = await Promise.allSettled([
    fetchJson(FNG_URL),
    fetchJson(BYBIT_RATIO_URL),
  ]);

  let fearGreed: FearGreed | null = null;
  if (fngResult.status === 'fulfilled') {
    try {
      fearGreed = parseFearGreed(fngResult.value);
    } catch (err) {
      errors.push(err instanceof Error ? err.message : String(err));
    }
  } else {
    errors.push(`fng: ${fngResult.reason}`);
  }

  let longShort: LongShort | null = null;
  if (ratioResult.status === 'fulfilled') {
    try {
      longShort = parseLongShort(ratioResult.value);
    } catch (err) {
      errors.push(err instanceof Error ? err.message : String(err));
    }
  } else {
    errors.push(`bybit: ${ratioResult.reason}`);
  }

  const body: SentimentResponse = { fearGreed, longShort, errors };
  const status = fearGreed || longShort ? 200 : 502;
  return NextResponse.json(body, { status });
}
