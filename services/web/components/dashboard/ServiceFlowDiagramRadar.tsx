'use client';

import { useMemo } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';

interface ServiceFlowDiagramProps {
  services?: Array<{ name: string; ok: boolean; latency_ms: number }>;
  executionMode?: 'paper' | 'testnet' | 'mainnet';
  executorState?: 'running' | 'paused' | 'down';
}

export function ServiceFlowDiagramRadar({ services = [], executionMode = 'paper', executorState = 'down' }: ServiceFlowDiagramProps) {
  const serviceMap = useMemo(() => new Map(services.map((svc) => [svc.name, svc])), [services]);
  
  const nodeOk = (name: string) => {
    const svc = serviceMap.get(name);
    if (svc) return svc.ok;
    if (name === 'executor') return executorState === 'running';
    return false;
  };

  const nodeLatency = (name: string) => serviceMap.get(name)?.latency_ms ?? 0;

  // Layout radial com executor no centro
  const centerNode = { name: 'executor', x: 50, y: 50 };
  
  const orbitNodes = [
    { name: 'strategy', angle: 180, distance: 25 },
    { name: 'market-data', angle: 210, distance: 38 },
    { name: 'bybit', angle: 240, distance: 50 },
    { name: 'binance', angle: 270, distance: 50 },
    { name: 'okx', angle: 300, distance: 50 },
    { name: 'api', angle: 90, distance: 32 },
    { name: 'monitor', angle: 45, distance: 32 },
    { name: 'analytics', angle: 315, distance: 32 },
    { name: 'backtest', angle: 270, distance: 32 },
  ];

  const connections = [
    ['binance', 'market-data'],
    ['bybit', 'market-data'],
    ['okx', 'market-data'],
    ['market-data', 'strategy'],
    ['strategy', 'executor'],
    ['executor', 'api'],
    ['executor', 'monitor'],
    ['executor', 'analytics'],
    ['executor', 'backtest'],
  ];

  return (
    <Card className="bg-slate-900/50 border-slate-700/50">
      <CardHeader className="pb-3">
        <CardTitle className="text-base text-slate-200">Architecture Flow</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="w-full overflow-x-auto">
          <svg viewBox="0 0 600 500" className="w-full h-auto" style={{ minHeight: 400 }}>
            {/* Background radar rings */}
            {[100, 170, 240].map((radius, idx) => (
              <circle
                key={idx}
                cx="300"
                cy="250"
                r={radius}
                fill="none"
                stroke="#334155"
                strokeWidth="1"
                strokeDasharray="4 4"
                opacity="0.5"
              />
            ))}
            
            {/* Cross lines */}
            <line x1="300" y1="10" x2="300" y2="490" stroke="#334155" strokeWidth="1" opacity="0.3" />
            <line x1="10" y1="250" x2="590" y2="250" stroke="#334155" strokeWidth="1" opacity="0.3" />
            
            {/* Connection lines */}
            {connections.map(([from, to]) => {
              const fromOrbit = orbitNodes.find(n => n.name === from);
              const toOrbit = orbitNodes.find(n => n.name === to);
              const isFromCenter = from === 'executor';
              const isToCenter = to === 'executor';
              
              let x1, y1, x2, y2;
              
              if (isFromCenter) {
                x1 = 300; y1 = 250;
                const target = fromOrbit || toOrbit!;
                const rad = (target.angle - 90) * Math.PI / 180;
                x2 = 300 + Math.cos(rad) * target.distance * 4.8;
                y2 = 250 + Math.sin(rad) * target.distance * 4.8;
              } else if (isToCenter) {
                const source = fromOrbit!;
                x1 = 300 + Math.cos((source.angle - 90) * Math.PI / 180) * source.distance * 4.8;
                y1 = 250 + Math.sin((source.angle - 90) * Math.PI / 180) * source.distance * 4.8;
                x2 = 300; y2 = 250;
              } else {
                const source = fromOrbit!;
                const target = toOrbit!;
                x1 = 300 + Math.cos((source.angle - 90) * Math.PI / 180) * source.distance * 4.8;
                y1 = 250 + Math.sin((source.angle - 90) * Math.PI / 180) * source.distance * 4.8;
                x2 = 300 + Math.cos((target.angle - 90) * Math.PI / 180) * target.distance * 4.8;
                y2 = 250 + Math.sin((target.angle - 90) * Math.PI / 180) * target.distance * 4.8;
              }
              
              const isActive = nodeOk(from) && nodeOk(to);
              
              return (
                <line
                  key={`${from}-${to}`}
                  x1={x1}
                  y1={y1}
                  x2={x2}
                  y2={y2}
                  stroke="#475569"
                  strokeWidth={isActive ? 2 : 1}
                  opacity={isActive ? 0.8 : 0.4}
                />
              );
            })}
            
            {/* Center node (executor) */}
            <g>
              <circle
                cx="300"
                cy="250"
                r="35"
                fill="#0f172a"
                stroke={executorState === 'running' ? '#10b981' : executorState === 'paused' ? '#f59e0b' : '#ef4444'}
                strokeWidth="2"
              />
              <circle
                cx="300"
                cy="250"
                r="5"
                fill={executorState === 'running' ? '#10b981' : executorState === 'paused' ? '#f59e0b' : '#ef4444'}
              />
              <text x="300" y="300" textAnchor="middle" fill="#94a3b8" fontSize="11" fontWeight="500">
                EXECUTOR
              </text>
            </g>
            
            {/* Orbit nodes */}
            {orbitNodes.map((node) => {
              const ok = nodeOk(node.name);
              const latency = nodeLatency(node.name);
              const rad = (node.angle - 90) * Math.PI / 180;
              const x = 300 + Math.cos(rad) * node.distance * 4.8;
              const y = 250 + Math.sin(rad) * node.distance * 4.8;
              const isExchange = ['bybit', 'binance', 'okx'].includes(node.name);
              
              return (
                <g key={node.name}>
                  <circle
                    cx={x}
                    cy={y}
                    r={isExchange ? 20 : 18}
                    fill="#0f172a"
                    stroke={ok ? '#10b981' : '#ef4444'}
                    strokeWidth="2"
                  />
                  <circle
                    cx={x}
                    cy={y}
                    r="4"
                    fill={ok ? '#10b981' : '#ef4444'}
                  />
                  <text
                    x={x}
                    y={y + (isExchange ? 35 : 32)}
                    textAnchor="middle"
                    fill="#94a3b8"
                    fontSize="10"
                    fontWeight="500"
                  >
                    {node.name.toUpperCase()}
                  </text>
                  {latency > 0 && (
                    <text
                      x={x}
                      y={y + (isExchange ? 48 : 45)}
                      textAnchor="middle"
                      fill="#64748b"
                      fontSize="9"
                    >
                      {latency}ms
                    </text>
                  )}
                </g>
              );
            })}
          </svg>
        </div>
        
        {/* Legend */}
        <div className="flex items-center gap-4 mt-2 text-xs text-slate-400">
          <div className="flex items-center gap-1.5">
            <span className="w-2 h-2 rounded-full bg-emerald-500" />
            <span>Healthy</span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="w-2 h-2 rounded-full bg-red-500" />
            <span>Unhealthy</span>
          </div>
          <div className="flex items-center gap-1.5 ml-auto">
            <span className="w-2 h-2 rounded-full bg-amber-500" />
            <span>Executor</span>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
