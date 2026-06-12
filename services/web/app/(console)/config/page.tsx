'use client';

import { useMemo, useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { ConfirmAction } from '@/components/system/ConfirmAction';
import { cn } from '@/lib/utils';

type Scalar = number | string | boolean;
type Json = Record<string, unknown>;

type VersionMeta = {
  id: number;
  created_at: string;
  created_by: string;
  note: string | null;
  active: boolean;
};
type ActiveConfig = {
  id: number;
  created_at: string;
  created_by: string;
  note: string | null;
  config: Json;
};
type ConfigState = { active: ActiveConfig | null; versions: VersionMeta[] };

const isScalar = (v: unknown): v is Scalar =>
  ['number', 'string', 'boolean'].includes(typeof v);
const prettify = (k: string) =>
  k.replaceAll('_', ' ').replace(/\b\w/g, c => c.toUpperCase());

function relTime(iso: string) {
  const ms = Date.parse(iso);
  if (Number.isNaN(ms)) return '—';
  const s = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

// Deep get/set on the plain config object using a path array.
function getPath(obj: unknown, path: string[]): unknown {
  return path.reduce<unknown>(
    (o, k) => (o && typeof o === 'object' ? (o as Json)[k] : undefined),
    obj
  );
}
function setPath(obj: Json, path: string[], value: unknown) {
  let o: Json = obj;
  for (let i = 0; i < path.length - 1; i++) o = o[path[i]] as Json;
  o[path[path.length - 1]] = value;
}

const PAPER_PATH = ['global', 'mode_profiles', 'PAPER'];

// Audited 2026-06-12 against every service: `name` is a profile label (shown
// read-only in the header, not a knob) and `prefer_bybit_for_decisions` is dead
// config (documented but read by no service). Keep them out of the edit grid.
const NON_TUNABLE = new Set(['name', 'prefer_bybit_for_decisions']);

// Flatten PAPER's scalar children (+ the thesis_health sub-object) into editable
// fields. relPath is relative to mode_profiles.PAPER.
type Field = {
  relPath: string[];
  label: string;
  value: Scalar;
  group: string;
};
function paperFields(config: Json): Field[] {
  const paper = getPath(config, PAPER_PATH);
  if (!paper || typeof paper !== 'object') return [];
  const out: Field[] = [];
  for (const [k, v] of Object.entries(paper as Json)) {
    if (k === 'thesis_health' && v && typeof v === 'object') {
      for (const [tk, tv] of Object.entries(v as Json)) {
        if (isScalar(tv))
          out.push({
            relPath: ['thesis_health', tk],
            label: prettify(tk),
            value: tv,
            group: 'Thesis health',
          });
      }
    } else if (isScalar(v) && !NON_TUNABLE.has(k)) {
      out.push({ relPath: [k], label: prettify(k), value: v, group: 'Tunables' });
    }
  }
  return out;
}

function FieldInput({
  field,
  draft,
  onChange,
}: {
  field: Field;
  draft: string | undefined;
  onChange: (v: string) => void;
}) {
  const current = draft ?? String(field.value);
  const dirty = draft !== undefined && draft !== String(field.value);
  const base =
    'rounded-md border bg-card px-2 py-1 text-sm text-foreground outline-none transition-colors focus:border-primary/50';
  const ring = dirty ? 'border-accent/60' : 'border-border';

  if (typeof field.value === 'boolean') {
    return (
      <select
        value={current}
        onChange={e => onChange(e.target.value)}
        className={cn(base, ring)}
      >
        <option value="true">true</option>
        <option value="false">false</option>
      </select>
    );
  }
  if (field.relPath.at(-1) === 'opposite_side_exit') {
    return (
      <select
        value={current}
        onChange={e => onChange(e.target.value)}
        className={cn(base, ring)}
      >
        {['any', 'both', 'off'].map(o => (
          <option key={o} value={o}>
            {o}
          </option>
        ))}
      </select>
    );
  }
  return (
    <input
      type={typeof field.value === 'number' ? 'number' : 'text'}
      step="any"
      value={current}
      onChange={e => onChange(e.target.value)}
      className={cn(base, ring, 'w-32 font-mono tabular-nums')}
    />
  );
}

export default function ConfigPage() {
  const { data, loading, error, refresh } = useDashboard<ConfigState>(
    '/api/v1/config',
    { refreshInterval: 0 }
  );
  const active = data?.active ?? null;
  const versions = data?.versions ?? [];

  const fields = useMemo(
    () => (active ? paperFields(active.config) : []),
    [active]
  );
  const profileName = active
    ? (getPath(active.config, [...PAPER_PATH, 'name']) as string | undefined)
    : undefined;
  const tokens = useMemo(() => {
    if (!active) return [];
    // Real token blocks have an `enabled` flag; this skips `global` and the
    // `profiles` risk-profile block, which ride along as top-level keys.
    return Object.keys(active.config)
      .filter(
        k => typeof (active.config[k] as Json)?.enabled === 'boolean'
      )
      .sort()
      .map(sym => ({
        sym,
        enabled: (active.config[sym] as Json)?.enabled !== false,
      }));
  }, [active]);

  // Pending edits: PAPER fields keyed by relPath.join('.'); tokens by symbol.
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [tokenDrafts, setTokenDrafts] = useState<Record<string, boolean>>({});
  const [pageError, setPageError] = useState<string | null>(null);

  // Add-token flow.
  type Avail = {
    symbol: string;
    bybit: boolean;
    binance: boolean;
    okx: boolean;
    available_on_all: boolean;
  };
  const [addSymbol, setAddSymbol] = useState('');
  const [cloneFrom, setCloneFrom] = useState('');
  const [avail, setAvail] = useState<Avail | null>(null);
  const [checking, setChecking] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);

  const dirtyCount =
    Object.entries(drafts).filter(([key, v]) => {
      const f = fields.find(ff => ff.relPath.join('.') === key);
      return f && v !== String(f.value);
    }).length +
    Object.entries(tokenDrafts).filter(([sym, v]) => {
      const t = tokens.find(tt => tt.sym === sym);
      return t && v !== t.enabled;
    }).length;

  function coerce(field: Field, raw: string): Scalar {
    if (typeof field.value === 'number') return Number(raw);
    if (typeof field.value === 'boolean') return raw === 'true';
    return raw;
  }

  async function send(kind: string, payload: Json) {
    const res = await fetch('/api/config', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ kind, payload }),
    });
    const body = await res.json().catch(() => null);
    if (!res.ok || body?.ok === false) {
      throw new Error(body?.message || body?.error || `HTTP ${res.status}`);
    }
    return body;
  }

  async function saveEdits() {
    if (!active) return;
    setPageError(null);
    const next = structuredClone(active.config) as Json;
    for (const [key, raw] of Object.entries(drafts)) {
      const f = fields.find(ff => ff.relPath.join('.') === key);
      if (f && raw !== String(f.value)) {
        setPath(next, [...PAPER_PATH, ...f.relPath], coerce(f, raw));
      }
    }
    for (const [sym, enabled] of Object.entries(tokenDrafts)) {
      const t = tokens.find(tt => tt.sym === sym);
      if (t && enabled !== t.enabled) setPath(next, [sym, 'enabled'], enabled);
    }
    await send('save', { config: next, note: 'web edit' });
    setDrafts({});
    setTokenDrafts({});
    await refresh();
  }

  async function checkSymbol() {
    setChecking(true);
    setAddError(null);
    setAvail(null);
    try {
      const body = await send('validate-symbol', { symbol: addSymbol });
      setAvail(body.result as Avail);
    } catch (e) {
      setAddError(e instanceof Error ? e.message : String(e));
    } finally {
      setChecking(false);
    }
  }

  async function addToken() {
    setAddError(null);
    const clone = cloneFrom || tokens[0]?.sym;
    await send('add-token', { symbol: addSymbol, clone_from: clone });
    setAddSymbol('');
    setAvail(null);
    await refresh();
  }

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">
            Config
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            The live trading config (PAPER profile + token universe). Edits create a
            new version and hot-reload the running strategy — no restart.
          </p>
        </div>
        {active && (
          <span className="font-mono text-xs text-muted-foreground">
            active v{active.id} · by {active.created_by} ·{' '}
            {relTime(active.created_at)}
          </span>
        )}
      </div>

      {(error || pageError) && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {pageError || error}
        </div>
      )}

      {loading && !active ? (
        <div className="h-72 animate-pulse rounded-xl border border-border bg-card" />
      ) : !active ? (
        <div className="flex h-40 items-center justify-center rounded-xl border border-border bg-card text-sm text-muted-foreground">
          No config version found.
        </div>
      ) : (
        <>
          {/* Tokens */}
          <section className="rounded-xl border border-border bg-card p-5">
            <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
              Token universe
            </div>
            <div className="grid grid-cols-2 gap-x-6 gap-y-1.5 sm:grid-cols-3 lg:grid-cols-4">
              {tokens.map(t => {
                const enabled = tokenDrafts[t.sym] ?? t.enabled;
                const dirty =
                  tokenDrafts[t.sym] !== undefined && enabled !== t.enabled;
                return (
                  <button
                    key={t.sym}
                    type="button"
                    onClick={() =>
                      setTokenDrafts(d => ({ ...d, [t.sym]: !enabled }))
                    }
                    className={cn(
                      'flex items-center justify-between gap-2 rounded-md border px-2.5 py-1.5 text-sm transition-colors',
                      enabled
                        ? 'border-accent/40 bg-accent/10 text-foreground'
                        : 'border-border bg-secondary/40 text-muted-foreground',
                      dirty && 'ring-1 ring-accent/60'
                    )}
                  >
                    <span className="font-mono">{t.sym}</span>
                    <span
                      className={cn(
                        'text-[10px] uppercase tracking-wide',
                        enabled ? 'text-accent' : 'text-muted-foreground'
                      )}
                    >
                      {enabled ? 'on' : 'off'}
                    </span>
                  </button>
                );
              })}
            </div>

            {/* add token */}
            <div className="mt-4 border-t border-border/50 pt-4">
              <div className="mb-2 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                Add token
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <input
                  value={addSymbol}
                  onChange={e => {
                    setAddSymbol(e.target.value.toUpperCase());
                    setAvail(null);
                    setAddError(null);
                  }}
                  placeholder="SOLUSDT"
                  className="w-32 rounded-md border border-border bg-card px-2 py-1 font-mono text-sm uppercase text-foreground outline-none transition-colors placeholder:normal-case focus:border-primary/50"
                />
                <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
                  clone from
                  <select
                    value={cloneFrom || tokens[0]?.sym || ''}
                    onChange={e => setCloneFrom(e.target.value)}
                    className="rounded-md border border-border bg-card px-2 py-1 text-sm text-foreground outline-none focus:border-primary/50"
                  >
                    {tokens.map(t => (
                      <option key={t.sym} value={t.sym}>
                        {t.sym}
                      </option>
                    ))}
                  </select>
                </label>
                <button
                  type="button"
                  disabled={!addSymbol || checking}
                  onClick={checkSymbol}
                  className="rounded-md border border-border px-3 py-1 text-xs text-foreground transition-colors hover:border-primary/40 disabled:opacity-50"
                >
                  {checking ? 'checking…' : 'Check'}
                </button>
                {avail && (
                  <span className="flex items-center gap-2.5 text-xs">
                    {(['bybit', 'binance', 'okx'] as const).map(ex => (
                      <span
                        key={ex}
                        className={cn(
                          'font-mono',
                          avail[ex] ? 'text-accent' : 'text-destructive'
                        )}
                      >
                        {avail[ex] ? '✓' : '✗'} {ex}
                      </span>
                    ))}
                  </span>
                )}
                {avail?.available_on_all && (
                  <ConfirmAction
                    label="Add (disabled)"
                    confirmLabel="Add token"
                    tone="danger"
                    onConfirm={addToken}
                  />
                )}
                {avail && !avail.available_on_all && (
                  <span className="text-xs text-destructive">
                    not on all 3 exchanges — can&apos;t add
                  </span>
                )}
              </div>
              <p className="mt-2 text-[11px] text-muted-foreground">
                Verified on Bybit + Binance + OKX (the consensus venues). Added
                disabled and cloned from the chosen token — review &amp; enable it
                above.
              </p>
              {addError && (
                <p className="mt-1 text-xs text-destructive">{addError}</p>
              )}
            </div>
          </section>

          {/* Tunables */}
          <section className="rounded-xl border border-border bg-card p-5">
            <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
              <h2 className="flex items-baseline gap-2 text-base font-semibold text-foreground">
                PAPER tunables
                {profileName && (
                  <span className="font-mono text-xs font-normal text-muted-foreground">
                    {profileName}
                  </span>
                )}
              </h2>
              <div className="flex items-center gap-3">
                {dirtyCount > 0 && (
                  <>
                    <span className="text-xs text-accent">
                      {dirtyCount} unsaved
                    </span>
                    <button
                      type="button"
                      onClick={() => {
                        setDrafts({});
                        setTokenDrafts({});
                      }}
                      className="rounded-md border border-border px-2 py-1 text-xs text-muted-foreground hover:text-foreground"
                    >
                      Discard
                    </button>
                    <ConfirmAction
                      label="Save changes"
                      confirmLabel="Apply live"
                      tone="danger"
                      onConfirm={saveEdits}
                    />
                  </>
                )}
              </div>
            </div>
            {['Tunables', 'Thesis health'].map(group => {
              const groupFields = fields.filter(f => f.group === group);
              if (groupFields.length === 0) return null;
              return (
                <div key={group} className="mb-4 last:mb-0">
                  <div className="mb-2 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    {group}
                  </div>
                  <div className="grid grid-cols-1 gap-x-6 gap-y-2 sm:grid-cols-2 lg:grid-cols-3">
                    {groupFields.map(f => {
                      const key = f.relPath.join('.');
                      return (
                        <label
                          key={key}
                          className="flex items-center justify-between gap-2 text-sm"
                        >
                          <span className="truncate text-muted-foreground">
                            {f.label}
                          </span>
                          <FieldInput
                            field={f}
                            draft={drafts[key]}
                            onChange={v =>
                              setDrafts(d => ({ ...d, [key]: v }))
                            }
                          />
                        </label>
                      );
                    })}
                  </div>
                </div>
              );
            })}
          </section>

          {/* Versions + promote */}
          <section className="rounded-xl border border-border bg-card p-5">
            <div className="mb-3 flex items-center justify-between">
              <div className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                Version history
              </div>
              <ConfirmAction
                label="Promote PAPER → MAINNET"
                confirmLabel="Promote"
                tone="danger"
                onConfirm={async () => {
                  setPageError(null);
                  try {
                    await send('promote', {});
                    await refresh();
                  } catch (e) {
                    setPageError(e instanceof Error ? e.message : String(e));
                  }
                }}
              />
            </div>
            <div className="divide-y divide-border">
              {versions.map(v => (
                <div
                  key={v.id}
                  className="flex flex-wrap items-center justify-between gap-2 py-2 text-sm"
                >
                  <span className="flex items-center gap-3">
                    <span className="font-mono text-muted-foreground">
                      v{v.id}
                    </span>
                    {v.active && (
                      <span className="rounded-md border border-accent/40 bg-accent/10 px-2 py-0.5 text-[11px] text-accent">
                        active
                      </span>
                    )}
                    <span className="text-xs text-muted-foreground">
                      {v.created_by} · {relTime(v.created_at)}
                      {v.note ? ` · ${v.note}` : ''}
                    </span>
                  </span>
                  {!v.active && (
                    <ConfirmAction
                      label="Activate"
                      confirmLabel="Revert to this"
                      tone="danger"
                      onConfirm={async () => {
                        setPageError(null);
                        try {
                          await send('activate', { version_id: v.id });
                          await refresh();
                        } catch (e) {
                          setPageError(
                            e instanceof Error ? e.message : String(e)
                          );
                        }
                      }}
                    />
                  )}
                </div>
              ))}
            </div>
          </section>
        </>
      )}
    </div>
  );
}
