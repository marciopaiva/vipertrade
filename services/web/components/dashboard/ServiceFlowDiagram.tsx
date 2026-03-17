'use client';

import { useMemo } from 'react';
import { cn } from '@/lib/utils';

interface ServiceFlowDiagramProps {
  services?: Array<{ name: string; ok: boolean; latency_ms: number }>;
  executionMode?: 'paper' | 'testnet' | 'mainnet';
  executorState?: 'running' | 'paused' | 'down';
  events?: Array<{ event_id: string; event_type: string; severity: string; timestamp: string; symbol?: string }>;
}

function serviceColor(name: string, ok: boolean): string {
  if (!ok) return '#ff6478';
  if (name.includes('bybit')) return '#f7b500';
  if (name === 'api') return '#11c4ff';
  if (name === 'executor') return '#00e5a8';
  if (name === 'analytics') return '#7bc4ff';
  if (name === 'strategy') return '#9580ff';
  return '#46d8ff';
}

export function ServiceFlowDiagram({ services = [], executionMode = 'paper', executorState = 'down', events = [] }: ServiceFlowDiagramProps) {
  const serviceMap = useMemo(() => new Map(services.map((svc) => [svc.name, svc])), [services]);

  const nodes = executionMode === 'testnet'
    ? [
        { name: 'bybit', x: 130, y: 250 },
        { name: 'market-data', x: 360, y: 250 },
        { name: 'strategy', x: 585, y: 250 },
        { name: 'executor', x: 790, y: 250 },
        { name: 'api', x: 1092, y: 86 },
        { name: 'monitor', x: 1092, y: 194 },
        { name: 'analytics', x: 1092, y: 302 },
        { name: 'backtest', x: 1092, y: 410 },
      ]
    : [
        { name: 'binance', x: 110, y: 140 },
        { name: 'bybit', x: 110, y: 280 },
        { name: 'okx', x: 110, y: 420 },
        { name: 'market-data', x: 360, y: 280 },
        { name: 'strategy', x: 610, y: 280 },
        { name: 'executor', x: 820, y: 280 },
        { name: 'api', x: 1080, y: 118 },
        { name: 'monitor', x: 1080, y: 226 },
        { name: 'analytics', x: 1080, y: 334 },
        { name: 'backtest', x: 1080, y: 442 },
      ];

  const links = executionMode === 'testnet'
    ? [
        ['bybit', 'market-data', 0],
        ['market-data', 'strategy', 0],
        ['strategy', 'executor', 0],
        ['executor', 'api', -22],
        ['executor', 'monitor', -8],
        ['executor', 'analytics', 8],
        ['executor', 'backtest', 22],
      ]
    : [
        ['binance', 'market-data', -10],
        ['bybit', 'market-data', 0],
        ['okx', 'market-data', 10],
        ['market-data', 'strategy', 0],
        ['strategy', 'executor', 0],
        ['executor', 'api', -22],
        ['executor', 'monitor', -8],
        ['executor', 'analytics', 8],
        ['executor', 'backtest', 22],
      ];

  const nodeOk = (name: string) => {
    const svc = serviceMap.get(name);
    if (svc) return svc.ok;
    if (name === 'executor') return executorState === 'running';
    return false;
  };
  const nodeLatency = (name: string) => serviceMap.get(name)?.latency_ms ?? 0;
  const executorColor = executorState === 'running' ? '#38f9a5' : executorState === 'paused' ? '#ffd978' : '#ff8f8f';

  // Get current token from recent executor events
  const currentToken = useMemo(() => {
    const executorEvents = events.filter(e => e.event_type.includes('executor') && e.symbol);
    if (executorEvents.length > 0) {
      return executorEvents[0].symbol?.replace('USDT', '') || 'DOGE';
    }
    return 'DOGE';
  }, [events]);

  // Check if there's a recent signal (for pulse effect)
  const hasRecentSignal = useMemo(() => {
    const recentSignals = events.filter(e =>
      e.event_type.includes('strategy') ||
      e.event_type.includes('decision')
    );
    return recentSignals.length > 0;
  }, [events]);

  return (
    <div className="w-full overflow-x-auto">
      <svg
        viewBox={executionMode === 'testnet' ? '0 0 1200 500' : '0 0 1180 560'}
        className="w-full h-auto"
        style={{ minHeight: 320 }}
      >
        <defs>
          <radialGradient id="viper-hub-glow" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stopColor="rgba(0,229,168,0.2)" />
            <stop offset="100%" stopColor="rgba(0,229,168,0)" />
          </radialGradient>
        </defs>

        {/* Glow no executor (opcional, mais sutil) */}
        <circle
          cx={executionMode === 'testnet' ? 790 : 820}
          cy={executionMode === 'testnet' ? 250 : 280}
          r={executionMode === 'testnet' ? 130 : 170}
          fill="url(#viper-hub-glow)"
        />

        {/* Links */}
        {links.map(([from, to, curve], idx) => {
          const fromNode = nodes.find(n => n.name === from);
          const toNode = nodes.find(n => n.name === to);
          if (!fromNode || !toNode) return null;

          const isActive = nodeOk(String(from)) && nodeOk(String(to));
          const color = serviceColor(String(to), isActive);

          const path = `M ${fromNode.x} ${fromNode.y} C ${fromNode.x + 140} ${Number(fromNode.y) + Number(curve)}, ${toNode.x - 140} ${Number(toNode.y) + Number(curve)}, ${toNode.x} ${toNode.y}`;

          return (
            <g key={`${from}-${to}`}>
              <path
                d={path}
                fill="none"
                stroke={isActive ? color : 'rgba(255,100,120,0.25)'}
                strokeWidth={isActive ? 1.8 : 1.2}
                opacity={isActive ? 0.55 : 0.25}
              />
              {isActive && (
                <path
                  d={path}
                  fill="none"
                  stroke={color}
                  strokeWidth={3}
                  opacity="0.2"
                  strokeDasharray="10 8"
                >
                  <animate
                    attributeName="stroke-dashoffset"
                    from="100"
                    to="0"
                    dur={`${2 + idx * 0.3}s`}
                    repeatCount="indefinite"
                  />
                </path>
              )}
            </g>
          );
        })}

        {/* Nodes */}
        {nodes.map((node) => {
          const ok = nodeOk(node.name);
          const isExecutor = node.name === 'executor';
          const color = isExecutor ? executorColor : serviceColor(node.name, ok);
          const latency = nodeLatency(node.name);

          return (
            <g key={node.name}>
              <circle
                cx={node.x}
                cy={node.y}
                r={20}
                fill="rgba(4,10,20,0.85)"
                stroke={color}
                strokeWidth={ok ? 2 : 1}
              />
              {ok && (
                <circle
                  cx={node.x}
                  cy={node.y}
                  r={26}
                  fill="none"
                  stroke={color}
                  strokeWidth={1}
                  opacity={0.35}
                >
                  <animate
                    attributeName="r"
                    values="26;32;26"
                    dur="2s"
                    repeatCount="indefinite"
                  />
                  <animate
                    attributeName="opacity"
                    values="0.35;0.15;0.35"
                    dur="2s"
                    repeatCount="indefinite"
                  />
                </circle>
              )}

              {/* Token symbol inside executor */}
              {isExecutor && ok && (
                <>
                  <text
                    x={node.x}
                    y={node.y - 8}
                    textAnchor="middle"
                    fill={color}
                    fontSize="14"
                    fontWeight="700"
                  >
                    {currentToken}
                  </text>
                  {/* Signal pulse ring */}
                  {hasRecentSignal && (
                    <circle
                      cx={node.x}
                      cy={node.y}
                      r="35"
                      fill="none"
                      stroke="#a855f7"
                      strokeWidth="2"
                      opacity="0.8"
                    >
                      <animate
                        attributeName="r"
                        values="35;50"
                        dur="1.5s"
                        repeatCount="indefinite"
                      />
                      <animate
                        attributeName="opacity"
                        values="0.8;0"
                        dur="1.5s"
                        repeatCount="indefinite"
                      />
                    </circle>
                  )}
                </>
              )}

              <text
                x={node.x}
                y={node.y + 42}
                textAnchor="middle"
                fill={ok ? '#d7e4ff' : '#8da7d7'}
                fontSize={11}
                fontWeight={600}
                style={{ textTransform: 'uppercase', letterSpacing: '0.5px' }}
              >
                {node.name.replace('-', ' ')}
              </text>
              {latency > 0 && (
                <text
                  x={node.x}
                  y={node.y + 56}
                  textAnchor="middle"
                  fill="#8da7d7"
                  fontSize={10}
                >
                  {latency}ms
                </text>
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
}
