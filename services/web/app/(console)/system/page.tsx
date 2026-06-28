'use client';

import { useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { useT } from '@/lib/i18n';
import { HudFrame } from '@/components/ui/HudFrame';
import { ConfirmAction } from '@/components/system/ConfirmAction';
import ServiceFlowDiagram from '@/components/dashboard/ServiceFlowDiagram';
import { cn } from '@/lib/utils';

interface ControlActor {
  enabled: boolean;
  reason?: string | null;
  actor?: string | null;
  updated_at?: string | null;
}
interface ControlState {
  operator_auth_mode?: string;
  operator_controls_enabled?: boolean;
  kill_switch?: ControlActor;
  executor?: ControlActor;
}
interface SysDash {
  status?: { trading_mode?: string };
  services?: Array<{ name: string; ok: boolean; latency_ms: number }>;
}

const FLAG_CMD =
  `kubectl patch configmap vipertrade-config -n vipertrade --type merge ` +
  `-p '{"data":{"STRATEGY_REAL_DECISIONS":"1"}}' && ` +
  `kubectl rollout restart deployment strategy -n vipertrade`;

async function sendControl(kind: string, payload: Record<string, unknown>) {
  const res = await fetch('/api/control', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ kind, payload }),
  });
  const data = await res.json().catch(() => null);
  if (!res.ok || data?.ok === false) {
    throw new Error(data?.message || data?.error || `HTTP ${res.status}`);
  }
}

function modeTone(mode: string) {
  if (mode === 'mainnet')
    return 'border-destructive/40 bg-destructive/10 text-destructive';
  if (mode === 'testnet') return 'border-warn/40 bg-warn/10 text-warn';
  return 'border-accent/40 bg-accent/10 text-accent';
}

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border/50 py-3 last:border-b-0">
      <div>
        <div className="text-sm font-medium text-foreground">{label}</div>
        {hint && <div className="text-xs text-muted-foreground">{hint}</div>}
      </div>
      <div className="flex items-center gap-3">{children}</div>
    </div>
  );
}

function StateDot({ ok, label }: { ok: boolean; label: string }) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 text-xs font-semibold',
        ok ? 'text-accent' : 'text-destructive'
      )}
    >
      <span
        className={cn(
          'h-2 w-2 rounded-full',
          ok ? 'bg-accent' : 'bg-destructive'
        )}
      />
      {label}
    </span>
  );
}

export default function SystemPage() {
  const t = useT('system');
  const { data: control, refresh } = useDashboard<ControlState>(
    '/api/v1/control/state',
    { refreshInterval: 10000 }
  );
  const { data: dash } = useDashboard<SysDash>('/api/dashboard', {
    refreshInterval: 5000,
  });
  const [copied, setCopied] = useState(false);

  const opEnabled = control?.operator_controls_enabled ?? false;
  const opReason = t('opDisabled');
  const killed = control?.kill_switch?.enabled ?? false;
  const executorOn = control?.executor?.enabled ?? false;
  const mode = (dash?.status?.trading_mode || 'paper').toLowerCase();
  const services = dash?.services ?? [];

  async function copyCmd() {
    try {
      await navigator.clipboard.writeText(FLAG_CMD);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard unavailable */
    }
  }

  return (
    <div className="space-y-5">
      <div>
        <h1 className="font-display text-2xl font-bold tracking-tight text-foreground">
          {t('title')}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {t('subtitlePre')}
          <code className="text-foreground/80">kubectl</code>
          {t('subtitlePost')}
        </p>
      </div>

      {!opEnabled && (
        <div className="rounded-md border border-warn/30 bg-warn/10 px-3 py-2 text-sm text-warn">
          {opReason} {t('opReadonly')}
        </div>
      )}

      {/* runtime */}
      <HudFrame title={t('runtime')}>
        <Row label={t('tradingMode')} hint={t('tradingModeHint')}>
          <span
            className={cn(
              'rounded-md border px-2.5 py-1 text-xs font-semibold uppercase tracking-wide',
              modeTone(mode)
            )}
          >
            {mode}
          </span>
        </Row>

        <Row
          label={t('killSwitch')}
          hint={
            killed
              ? control?.kill_switch?.actor
                ? t('killTrippedBy', { actor: control.kill_switch.actor })
                : t('killTripped')
              : t('killArmed')
          }
        >
          <StateDot ok={!killed} label={killed ? t('stTripped') : t('stArmed')} />
          {killed ? (
            <ConfirmAction
              label={t('restore')}
              confirmLabel={t('restoreTrading')}
              disabled={!opEnabled}
              disabledReason={opReason}
              onConfirm={async () => {
                await sendControl('kill-switch', {
                  enabled: false,
                  reason: 'web-operator restore',
                });
                await refresh();
              }}
            />
          ) : (
            <ConfirmAction
              label={t('tripKill')}
              confirmLabel={t('tripNow')}
              tone="danger"
              disabled={!opEnabled}
              disabledReason={opReason}
              onConfirm={async () => {
                await sendControl('kill-switch', {
                  enabled: true,
                  reason: 'web-operator trip',
                });
                await refresh();
              }}
            />
          )}
        </Row>

        <Row
          label={t('executor')}
          hint={executorOn ? t('executorProcessing') : t('executorPaused')}
        >
          <StateDot ok={executorOn} label={executorOn ? t('stOn') : t('stOff')} />
          <ConfirmAction
            label={executorOn ? t('disable') : t('enable')}
            confirmLabel={
              executorOn ? t('disableExecutor') : t('enableExecutor')
            }
            tone={executorOn ? 'danger' : 'default'}
            disabled={!opEnabled}
            disabledReason={opReason}
            onConfirm={async () => {
              await sendControl('executor', {
                enabled: !executorOn,
                reason: 'web-operator toggle',
              });
              await refresh();
            }}
          />
        </Row>

        <Row
          label="STRATEGY_REAL_DECISIONS"
          hint={t('realDecisionsHint')}
        >
          <button
            type="button"
            onClick={copyCmd}
            className="inline-flex items-center rounded-md border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:border-primary/40"
          >
            {copied ? t('copiedCmd') : t('copyCommand')}
          </button>
        </Row>
        <pre className="mt-2 overflow-x-auto rounded-md border border-border bg-secondary/40 p-3 font-mono text-[11px] leading-relaxed text-muted-foreground">
          {FLAG_CMD}
        </pre>
      </HudFrame>

      {/* service health — the architecture-flow pipeline (moved from /console) */}
      <HudFrame title={t('serviceHealth')}>
        {services.length === 0 ? (
          <div className="py-6 text-center text-sm text-muted-foreground">
            {t('noServiceHealth')}
          </div>
        ) : (
          <ServiceFlowDiagram
            services={services}
            executionMode={mode as 'paper' | 'testnet' | 'mainnet'}
            executorState={executorOn ? 'running' : 'down'}
          />
        )}
      </HudFrame>
    </div>
  );
}
