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
    } else if (isScalar(v)) {
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
  const tokens = useMemo(() => {
    if (!active) return [];
    return Object.keys(active.config)
      .filter(k => k !== 'global')
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
          {/* Tunables */}
          <section className="rounded-xl border border-border bg-card p-5">
            <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
              <h2 className="text-base font-semibold text-foreground">
                PAPER tunables
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

          {/* Tokens */}
          <section className="rounded-xl border border-border bg-card p-5">
            <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
              Token universe
            </div>
            <div className="grid grid-cols-2 gap-x-6 gap-y-1.5 sm:grid-cols-3 lg:grid-cols-4">
              {tokens.map(t => {
                const enabled = tokenDrafts[t.sym] ?? t.enabled;
                const dirty = tokenDrafts[t.sym] !== undefined && enabled !== t.enabled;
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
