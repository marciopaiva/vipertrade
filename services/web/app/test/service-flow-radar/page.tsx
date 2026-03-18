// services/web/app/test/service-flow-radar/page.tsx
'use client';

import { useState, useEffect } from 'react';
import { ServiceFlowDiagramRadar } from '@/components/dashboard/ServiceFlowDiagramRadar';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

interface Service {
  name: string;
  ok: boolean;
  latency_ms: number;
}

export default function ServiceFlowRadarTest() {
  const [services, setServices] = useState<Service[]>([
    { name: 'bybit', ok: true, latency_ms: 45 },
    { name: 'binance', ok: true, latency_ms: 52 },
    { name: 'okx', ok: true, latency_ms: 68 },
    { name: 'market-data', ok: true, latency_ms: 12 },
    { name: 'strategy', ok: true, latency_ms: 8 },
    { name: 'executor', ok: true, latency_ms: 9 },
    { name: 'api', ok: true, latency_ms: 11 },
    { name: 'monitor', ok: true, latency_ms: 7 },
    { name: 'analytics', ok: true, latency_ms: 10 },
    { name: 'backtest', ok: true, latency_ms: 15 },
  ]);

  const [executionMode, setExecutionMode] = useState<'paper' | 'testnet' | 'mainnet'>('paper');
  const [executorState, setExecutorState] = useState<'running' | 'paused' | 'down'>('running');

  // Simulate real-time latency updates
  useEffect(() => {
    const interval = setInterval(() => {
      setServices(prev => prev.map(service => ({
        ...service,
        latency_ms: Math.max(5, Math.floor(service.latency_ms + (Math.random() * 20 - 10))),
      })));
    }, 2000);

    return () => clearInterval(interval);
  }, []);

  // Simulate random service failures
  const simulateFailure = () => {
    const randomIndex = Math.floor(Math.random() * services.length);
    setServices(prev => prev.map((service, index) => 
      index === randomIndex ? { ...service, ok: !service.ok } : service
    ));
  };

  // Simulate executor state change
  const cycleExecutorState = () => {
    setExecutorState(prev => {
      if (prev === 'running') return 'paused';
      if (prev === 'paused') return 'down';
      return 'running';
    });
  };

  // Cycle execution mode
  const cycleExecutionMode = () => {
    setExecutionMode(prev => {
      if (prev === 'paper') return 'testnet';
      if (prev === 'testnet') return 'mainnet';
      return 'paper';
    });
  };

  return (
    <div className="min-h-screen bg-[#0a1929] p-8">
      <div className="max-w-7xl mx-auto space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-3xl font-bold text-white tracking-tight">
              ServiceFlowDiagramRadar - Test Container
            </h1>
            <p className="text-cyan-400/80 text-sm mt-1">
              Interactive testing environment for the radar visualization component
            </p>
          </div>
          <Badge variant="outline" className="text-cyan-400 border-cyan-400/50">
            v1.0.0
          </Badge>
        </div>

        {/* Control Panel */}
        <Card className="bg-slate-800/50 border-slate-700/50">
          <CardHeader className="pb-3">
            <CardTitle className="text-base text-slate-200">Control Panel</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex flex-wrap gap-3">
              {/* Execution Mode */}
              <div className="flex items-center gap-2">
                <span className="text-slate-400 text-sm">Mode:</span>
                <Button
                  onClick={cycleExecutionMode}
                  variant="outline"
                  size="sm"
                  className="border-purple-500/50 text-purple-400 hover:bg-purple-500/20"
                >
                  {executionMode.toUpperCase()}
                </Button>
              </div>

              {/* Executor State */}
              <div className="flex items-center gap-2">
                <span className="text-slate-400 text-sm">Executor:</span>
                <Button
                  onClick={cycleExecutorState}
                  variant="outline"
                  size="sm"
                  className={
                    executorState === 'running' ? 'border-green-500/50 text-green-400 hover:bg-green-500/20' :
                    executorState === 'paused' ? 'border-yellow-500/50 text-yellow-400 hover:bg-yellow-500/20' :
                    'border-red-500/50 text-red-400 hover:bg-red-500/20'
                  }
                >
                  {executorState.toUpperCase()}
                </Button>
              </div>

              {/* Simulate Failure */}
              <Button
                onClick={simulateFailure}
                variant="outline"
                size="sm"
                className="border-orange-500/50 text-orange-400 hover:bg-orange-500/20"
              >
                ⚡ Toggle Random Service
              </Button>

              {/* Reset */}
              <Button
                onClick={() => {
                  setServices(prev => prev.map(s => ({ ...s, ok: true })));
                  setExecutorState('running');
                  setExecutionMode('paper');
                }}
                variant="outline"
                size="sm"
                className="border-cyan-500/50 text-cyan-400 hover:bg-cyan-500/20"
              >
                🔄 Reset All
              </Button>
            </div>
          </CardContent>
        </Card>

        {/* Service Status Grid */}
        <Card className="bg-slate-800/50 border-slate-700/50">
          <CardHeader className="pb-3">
            <CardTitle className="text-base text-slate-200">Service Status</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 md:grid-cols-5 lg:grid-cols-10 gap-3">
              {services.map((service) => (
                <div
                  key={service.name}
                  className={`p-3 rounded-lg border text-center transition-all ${
                    service.ok
                      ? 'border-green-500/30 bg-green-500/10'
                      : 'border-red-500/30 bg-red-500/10'
                  }`}
                >
                  <div className="text-xs text-slate-400 capitalize truncate">
                    {service.name}
                  </div>
                  <div className={`text-sm font-semibold mt-1 ${
                    service.ok ? 'text-green-400' : 'text-red-400'
                  }`}>
                    {service.ok ? '✓' : '✗'}
                  </div>
                  <div className="text-xs text-slate-500 mt-1">
                    {service.latency_ms}ms
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        {/* Main Radar Visualization */}
        <ServiceFlowDiagramRadar
          services={services}
          executionMode={executionMode}
          executorState={executorState}
        />

        {/* Info Section */}
        <Card className="bg-slate-800/50 border-slate-700/50">
          <CardHeader className="pb-3">
            <CardTitle className="text-base text-slate-200">Component Info</CardTitle>
          </CardHeader>
          <CardContent className="text-sm text-slate-400 space-y-2">
            <p>
              <strong className="text-cyan-400">ServiceFlowDiagramRadar</strong> is an alternative 
              radial visualization component for the ViperTrade architecture flow.
            </p>
            <ul className="list-disc list-inside space-y-1 ml-2">
              <li>Central node: <strong className="text-green-400">Executor</strong></li>
              <li>Orbit nodes: Exchanges, Infrastructure, Processing, Monitoring</li>
              <li>Connection lines show data flow between services</li>
              <li>Green lines indicate healthy connections</li>
              <li>Red lines indicate service issues</li>
              <li>Real-time latency updates every 2 seconds</li>
            </ul>
          </CardContent>
        </Card>

        {/* Usage Example */}
        <Card className="bg-slate-800/50 border-slate-700/50">
          <CardHeader className="pb-3">
            <CardTitle className="text-base text-slate-200">Usage Example</CardTitle>
          </CardHeader>
          <CardContent>
            <pre className="bg-[#0a1929] p-4 rounded-lg overflow-x-auto text-xs text-slate-300">
{`import { ServiceFlowDiagramRadar } from '@/components/dashboard/ServiceFlowDiagramRadar';

<ServiceFlowDiagramRadar
  services={services}           // Array of service health data
  executionMode="paper"         // 'paper' | 'testnet' | 'mainnet'
  executorState="running"       // 'running' | 'paused' | 'down'
/>`}
            </pre>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
