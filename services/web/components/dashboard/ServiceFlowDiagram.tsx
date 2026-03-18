// components/dashboard/ServiceFlowDiagram.tsx
'use client';

import React, { useEffect, useRef, useState, useMemo } from 'react';

interface ServiceFlowDiagramProps {
  services?: Array<{ name: string; ok: boolean; latency_ms: number }>;
  executionMode?: 'paper' | 'testnet' | 'mainnet';
  executorState?: 'running' | 'paused' | 'down';
  events?: Array<{ event_id: string; event_type: string; severity: string; timestamp: string; symbol?: string }>;
  width?: number;
  height?: number;
  selectedSymbol?: string;
}

// 🎨 VIPERTRADE COLOR PALETTE
const COLORS = {
  // Background & Base
  background: '#0a1929',
  surface: '#0f2441',
  border: 'rgba(6, 182, 212, 0.2)',
  
  // Primary Brand Colors
  viperCyan: '#00d4ff',
  viperGreen: '#00ff88',
  viperPurple: '#a855f7',
  viperBlue: '#3b82f6',
  
  // Exchanges (Warm tones)
  binance: '#f0b90b',
  okx: '#ffffff',
  bybit: '#f7a600',
  
  // Infrastructure (Cool tones)
  marketData: '#06b6d4',
  analytics: '#0891b2',
  
  // Processing (Purple tones)
  strategy: '#a855f7',
  backtest: '#9333ea',
  
  // Executor (Green - maximum highlight)
  executor: '#00ff88',
  executorGlow: '#10b981',
  
  // Monitoring (Blue tones)
  api: '#3b82f6',
  monitor: '#00d4ff',
  
  // Status Indicators
  statusActive: '#10b981',
  statusWarning: '#f59e0b',
  statusError: '#ef4444',
  statusInactive: '#64748b',
  
  // Connections
  connectionPrimary: '#14b8a6',
  connectionSecondary: '#06b6d4',
  connectionAlert: '#f59e0b',
};

interface Node {
  id: string;
  label: string;
  sublabel?: string;
  x: number;
  y: number;
  status: 'active' | 'inactive' | 'warning' | 'error';
  latency?: number;
  color: string;
  size?: 'sm' | 'md' | 'lg' | 'xl';
  isCentral?: boolean;
}

interface Connection {
  from: string;
  to: string;
  color: string;
  animated?: boolean;
  delay?: number;
}

const getNodeSize = (size: string = 'md') => {
  const sizes = {
    sm: { outer: 25, inner: 20, center: 5 },
    md: { outer: 32, inner: 27, center: 7 },
    lg: { outer: 42, inner: 36, center: 9 },
    xl: { outer: 55, inner: 45, center: 12 },
  };
  return sizes[size as keyof typeof sizes] || sizes.md;
};

export default function ServiceFlowDiagram({
  services = [],
  executionMode = 'paper',
  executorState = 'down',
  events = [],
  width = 900,
  height = 400,
  selectedSymbol,
}: ServiceFlowDiagramProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);
  const [animationKey, setAnimationKey] = useState(0);

  // Create service map for quick lookups
  const serviceMap = useMemo(() => new Map(services.map((svc) => [svc.name, svc])), [services]);

  // Get node status from service health
  const getNodeStatus = (nodeId: string): 'active' | 'inactive' | 'warning' | 'error' => {
    const service = serviceMap.get(nodeId);
    if (!service) {
      if (nodeId === 'executor') {
        return executorState === 'running' ? 'active' : executorState === 'paused' ? 'warning' : 'error';
      }
      return 'inactive';
    }
    if (!service.ok) return 'error';
    if (service.latency_ms > 500) return 'warning';
    return 'active';
  };

  // Get node latency from service data
  const getNodeLatency = (nodeId: string): number => {
    const service = serviceMap.get(nodeId);
    return service?.latency_ms ?? 0;
  };

  // Get node color based on status and type
  const getNodeColor = (nodeId: string, status: string): string => {
    if (status === 'error') return COLORS.statusError;
    if (status === 'warning') return COLORS.statusWarning;
    
    const colorMap: Record<string, string> = {
      'binance': COLORS.binance,
      'okx': COLORS.okx,
      'bybit': COLORS.bybit,
      'market-data': COLORS.marketData,
      'analytics': COLORS.analytics,
      'strategy': COLORS.strategy,
      'executor': COLORS.executor,
      'api': COLORS.api,
      'monitor': COLORS.monitor,
      'backtest': COLORS.backtest,
    };
    
    return colorMap[nodeId] || COLORS.viperCyan;
  };

  // Get current token from recent executor events
  const currentToken = useMemo(() => {
    const executorEvents = events.filter(e => e.event_type.includes('executor') && e.symbol);
    if (executorEvents.length > 0) {
      return executorEvents[0].symbol?.replace('USDT', '') || 'DOGE';
    }
    return selectedSymbol || 'DOGE';
  }, [events, selectedSymbol]);

  // Check if there's a recent signal (for pulse effect)
  const hasRecentSignal = useMemo(() => {
    const recentSignals = events.filter(e =>
      e.event_type.includes('strategy') ||
      e.event_type.includes('decision')
    );
    return recentSignals.length > 0;
  }, [events]);

  // Re-trigger animations periodically
  useEffect(() => {
    const interval = setInterval(() => {
      setAnimationKey(prev => prev + 1);
    }, 8000);
    return () => clearInterval(interval);
  }, []);

  // Dynamic nodes based on execution mode
  const NODES: Node[] = executionMode === 'testnet'
    ? [
        // Testnet mode - simplified (Bybit only)
        { id: 'bybit', label: 'BYBIT', sublabel: `${getNodeLatency('bybit')}ms`, x: 80, y: 200, status: getNodeStatus('bybit'), latency: getNodeLatency('bybit'), color: getNodeColor('bybit', getNodeStatus('bybit')), size: 'md' as const },
        { id: 'market-data', label: 'MARKET-DATA', sublabel: `${getNodeLatency('market-data')}ms`, x: 280, y: 200, status: getNodeStatus('market-data'), latency: getNodeLatency('market-data'), color: getNodeColor('market-data', getNodeStatus('market-data')), size: 'md' as const },
        { id: 'strategy', label: 'STRATEGY', sublabel: `${getNodeLatency('strategy')}ms`, x: 420, y: 200, status: getNodeStatus('strategy'), latency: getNodeLatency('strategy'), color: getNodeColor('strategy', getNodeStatus('strategy')), size: 'md' as const },
        { id: 'executor', label: 'EXECUTOR', sublabel: `${getNodeLatency('executor')}ms • ${currentToken}`, x: 620, y: 200, status: getNodeStatus('executor'), latency: getNodeLatency('executor'), color: getNodeColor('executor', getNodeStatus('executor')), size: 'xl' as const, isCentral: true },
        { id: 'api', label: 'API', sublabel: `${getNodeLatency('api')}ms`, x: 820, y: 80, status: getNodeStatus('api'), latency: getNodeLatency('api'), color: getNodeColor('api', getNodeStatus('api')), size: 'md' as const },
        { id: 'monitor', label: 'MONITOR', sublabel: `${getNodeLatency('monitor')}ms`, x: 820, y: 200, status: getNodeStatus('monitor'), latency: getNodeLatency('monitor'), color: getNodeColor('monitor', getNodeStatus('monitor')), size: 'md' as const },
        { id: 'analytics', label: 'ANALYTICS', sublabel: `${getNodeLatency('analytics')}ms`, x: 820, y: 320, status: getNodeStatus('analytics'), latency: getNodeLatency('analytics'), color: getNodeColor('analytics', getNodeStatus('analytics')), size: 'md' as const },
        { id: 'backtest', label: 'BACKTEST', sublabel: `${getNodeLatency('backtest')}ms`, x: 820, y: 440, status: getNodeStatus('backtest'), latency: getNodeLatency('backtest'), color: getNodeColor('backtest', getNodeStatus('backtest')), size: 'md' as const },
      ]
    : [
        // Mainnet/Paper mode - full (multi-exchange)
        { id: 'binance', label: 'BINANCE', sublabel: `${getNodeLatency('binance')}ms`, x: 80, y: 60, status: getNodeStatus('binance'), latency: getNodeLatency('binance'), color: getNodeColor('binance', getNodeStatus('binance')), size: 'md' as const },
        { id: 'okx', label: 'OKX', sublabel: `${getNodeLatency('okx')}ms`, x: 80, y: 200, status: getNodeStatus('okx'), latency: getNodeLatency('okx'), color: getNodeColor('okx', getNodeStatus('okx')), size: 'md' as const },
        { id: 'bybit', label: 'BYBIT', sublabel: `${getNodeLatency('bybit')}ms`, x: 80, y: 340, status: getNodeStatus('bybit'), latency: getNodeLatency('bybit'), color: getNodeColor('bybit', getNodeStatus('bybit')), size: 'md' as const },
        { id: 'market-data', label: 'MARKET-DATA', sublabel: `${getNodeLatency('market-data')}ms`, x: 280, y: 100, status: getNodeStatus('market-data'), latency: getNodeLatency('market-data'), color: getNodeColor('market-data', getNodeStatus('market-data')), size: 'md' as const },
        { id: 'analytics', label: 'ANALYTICS', sublabel: `${getNodeLatency('analytics')}ms`, x: 280, y: 300, status: getNodeStatus('analytics'), latency: getNodeLatency('analytics'), color: getNodeColor('analytics', getNodeStatus('analytics')), size: 'md' as const },
        { id: 'strategy', label: 'STRATEGY', sublabel: `${getNodeLatency('strategy')}ms`, x: 420, y: 200, status: getNodeStatus('strategy'), latency: getNodeLatency('strategy'), color: getNodeColor('strategy', getNodeStatus('strategy')), size: 'md' as const },
        { id: 'executor', label: 'EXECUTOR', sublabel: `${getNodeLatency('executor')}ms • ${currentToken}`, x: 620, y: 200, status: getNodeStatus('executor'), latency: getNodeLatency('executor'), color: getNodeColor('executor', getNodeStatus('executor')), size: 'xl' as const, isCentral: true },
        { id: 'api', label: 'API', sublabel: `${getNodeLatency('api')}ms`, x: 820, y: 60, status: getNodeStatus('api'), latency: getNodeLatency('api'), color: getNodeColor('api', getNodeStatus('api')), size: 'md' as const },
        { id: 'monitor', label: 'MONITOR', sublabel: `${getNodeLatency('monitor')}ms`, x: 820, y: 200, status: getNodeStatus('monitor'), latency: getNodeLatency('monitor'), color: getNodeColor('monitor', getNodeStatus('monitor')), size: 'md' as const },
        { id: 'backtest', label: 'BACKTEST', sublabel: `${getNodeLatency('backtest')}ms`, x: 820, y: 340, status: getNodeStatus('backtest'), latency: getNodeLatency('backtest'), color: getNodeColor('backtest', getNodeStatus('backtest')), size: 'md' as const },
      ];

  const CONNECTIONS: Connection[] = executionMode === 'testnet'
    ? [
        { from: 'bybit', to: 'market-data', color: COLORS.bybit, animated: true, delay: 0 },
        { from: 'market-data', to: 'strategy', color: COLORS.viperPurple, animated: true, delay: 0 },
        { from: 'strategy', to: 'executor', color: COLORS.viperGreen, animated: true, delay: 0 },
        { from: 'executor', to: 'api', color: COLORS.api, animated: true, delay: 0 },
        { from: 'executor', to: 'monitor', color: COLORS.viperCyan, animated: true, delay: 0.5 },
        { from: 'executor', to: 'analytics', color: COLORS.analytics, animated: true, delay: 1 },
        { from: 'executor', to: 'backtest', color: COLORS.backtest, animated: true, delay: 1.5 },
      ]
    : [
        // Exchanges → Market Data
        { from: 'binance', to: 'market-data', color: COLORS.connectionPrimary, animated: true, delay: 0 },
        { from: 'okx', to: 'market-data', color: COLORS.connectionPrimary, animated: true, delay: 0.5 },
        { from: 'bybit', to: 'market-data', color: COLORS.bybit, animated: true, delay: 1 },
        // Bybit → Analytics
        { from: 'bybit', to: 'analytics', color: COLORS.connectionSecondary, animated: true, delay: 1.5 },
        // Market Data → Strategy
        { from: 'market-data', to: 'strategy', color: COLORS.viperPurple, animated: true, delay: 0 },
        // Strategy → Executor
        { from: 'strategy', to: 'executor', color: COLORS.viperGreen, animated: true, delay: 0 },
        // Analytics → Executor
        { from: 'analytics', to: 'executor', color: COLORS.connectionPrimary, animated: true, delay: 0.5 },
        // Executor → Monitoring
        { from: 'executor', to: 'api', color: COLORS.api, animated: true, delay: 0 },
        { from: 'executor', to: 'monitor', color: COLORS.viperCyan, animated: true, delay: 0.5 },
        { from: 'executor', to: 'backtest', color: COLORS.backtest, animated: true, delay: 1 },
      ];

  return (
    <div className="relative w-full bg-[#0a1929] rounded-xl border border-cyan-900/30 overflow-hidden shadow-2xl">
      {/* Header */}
      <div className="absolute top-4 left-6 z-10">
        <h3 className="text-lg font-semibold text-white tracking-tight">System Architecture</h3>
        <p className="text-xs text-cyan-400/80 mt-0.5">Real-time data flow • Latency monitoring</p>
      </div>

      {/* Mode & Token Badge */}
      <div className="absolute top-4 right-6 z-10 flex items-center gap-2">
        <div className="flex items-center gap-2 px-3 py-1.5 bg-purple-500/10 border border-purple-500/40 rounded-full backdrop-blur-sm">
          <span className="text-purple-400 text-xs font-bold tracking-wide uppercase">{executionMode}</span>
        </div>
        <div className="flex items-center gap-2 px-4 py-2 bg-green-500/10 border border-green-500/40 rounded-full backdrop-blur-sm">
          <div className="w-2 h-2 bg-green-400 rounded-full animate-pulse shadow-lg shadow-green-400/50" />
          <span className="text-green-400 text-sm font-bold tracking-wide">{currentToken}</span>
        </div>
      </div>

      {/* SVG Canvas */}
      <svg
        ref={svgRef}
        viewBox="0 0 900 500"
        className="w-full h-auto"
        style={{ minHeight: '400px' }}
        preserveAspectRatio="xMinYMin meet"
      >
        <defs>
          {/* Glow Filter for Nodes */}
          <filter id="nodeGlow" x="-100%" y="-100%" width="300%" height="300%">
            <feGaussianBlur stdDeviation="4" result="coloredBlur" />
            <feMerge>
              <feMergeNode in="coloredBlur" />
              <feMergeNode in="coloredBlur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>

          {/* Strong Glow for Central Node */}
          <filter id="centralGlow" x="-150%" y="-150%" width="400%" height="400%">
            <feGaussianBlur stdDeviation="6" result="coloredBlur" />
            <feMerge>
              <feMergeNode in="coloredBlur" />
              <feMergeNode in="coloredBlur" />
              <feMergeNode in="coloredBlur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>

          {/* Glow Gradient for Central Node */}
          <radialGradient id="centralGlowGradient">
            <stop offset="0%" stopColor="#00ff88" stopOpacity="1" />
            <stop offset="50%" stopColor="#00d4ff" stopOpacity="0.6" />
            <stop offset="100%" stopColor="#0a1929" stopOpacity="0" />
          </radialGradient>

          {/* Gradient for Connection Lines */}
          <linearGradient id="lineGradient" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#14b8a6" stopOpacity="0.2" />
            <stop offset="50%" stopColor="#00d4ff" stopOpacity="0.8" />
            <stop offset="100%" stopColor="#14b8a6" stopOpacity="0.2" />
          </linearGradient>

          {/* Particle Gradient */}
          <radialGradient id="particleGradient">
            <stop offset="0%" stopColor="#ffffff" stopOpacity="1" />
            <stop offset="50%" stopColor="#00ff88" stopOpacity="0.9" />
            <stop offset="100%" stopColor="#00d4ff" stopOpacity="0" />
          </radialGradient>
        </defs>

        {/* Connection Lines */}
        <g className="connections" key={animationKey}>
          {CONNECTIONS.map((conn, index) => {
            const fromNode = NODES.find(n => n.id === conn.from);
            const toNode = NODES.find(n => n.id === conn.to);
            if (!fromNode || !toNode) return null;

            const fromSize = getNodeSize(fromNode.size);
            const toSize = getNodeSize(toNode.size);

            // Calculate curve control points for smooth bezier
            const midX = (fromNode.x + toNode.x) / 2;
            const controlPoint1X = fromNode.x + (midX - fromNode.x) * 0.7;
            const controlPoint2X = toNode.x - (toNode.x - midX) * 0.7;

            const pathData = `M ${fromNode.x + fromSize.outer} ${fromNode.y} 
                             C ${controlPoint1X} ${fromNode.y}, 
                               ${controlPoint2X} ${toNode.y}, 
                               ${toNode.x - toSize.outer} ${toNode.y}`;

            const isActive = fromNode.status === 'active' && toNode.status === 'active';

            return (
              <g key={`${conn.from}-${conn.to}`}>
                {/* Base Line (subtle) */}
                <path
                  d={pathData}
                  stroke={isActive ? conn.color : COLORS.statusInactive}
                  strokeWidth="1.5"
                  fill="none"
                  opacity={isActive ? 0.5 : 0.2}
                  strokeDasharray="4,4"
                />

                {/* Animated Particle */}
                {conn.animated && isActive && (
                  <>
                    <circle r="5" fill={conn.color} opacity="0.8" filter="url(#nodeGlow)">
                      <animateMotion
                        dur={`${3 + (conn.delay ?? 0)}s`}
                        repeatCount="indefinite"
                        path={pathData}
                      />
                    </circle>
                    <circle r="3" fill="#ffffff" opacity="0.6">
                      <animateMotion
                        dur={`${3 + (conn.delay ?? 0)}s`}
                        repeatCount="indefinite"
                        path={pathData}
                      />
                    </circle>
                  </>
                )}
              </g>
            );
          })}
        </g>

        {/* Nodes */}
        <g className="nodes">
          {NODES.map((node) => {
            const sizes = getNodeSize(node.size);
            const isHovered = hoveredNode === node.id;
            const isExecutor = node.id === 'executor';

            return (
              <g
                key={node.id}
                transform={`translate(${node.x}, ${node.y})`}
                onMouseEnter={() => setHoveredNode(node.id)}
                onMouseLeave={() => setHoveredNode(null)}
                className="cursor-pointer transition-all duration-300"
                style={{
                  filter: isHovered || isExecutor ? `url(#${isExecutor ? 'centralGlow' : 'nodeGlow'})` : 'none',
                  transform: isHovered ? 'scale(1.05)' : 'scale(1)',
                  transformOrigin: `${node.x}px ${node.y}px`,
                }}
              >
                {/* Outer Glow Ring (animated pulse) */}
                <circle
                  r={sizes.outer + 10}
                  fill="none"
                  stroke={node.color}
                  strokeWidth="1"
                  opacity="0.2"
                  className="animate-pulse"
                />

                {/* Main Outer Circle */}
                <circle
                  r={sizes.outer}
                  fill="#0a1929"
                  stroke={node.color}
                  strokeWidth={isExecutor ? 3 : 2}
                  opacity={isExecutor ? 1 : 0.9}
                />

                {/* Inner Ring */}
                <circle
                  r={sizes.inner}
                  fill="none"
                  stroke={node.color}
                  strokeWidth="1"
                  opacity="0.5"
                />

                {/* Center Dot */}
                <circle
                  r={sizes.center}
                  fill={node.color}
                  opacity="0.9"
                  filter={isExecutor ? 'url(#centralGlow)' : 'none'}
                />

                {/* Status Indicator (top-right) */}
                <circle
                  cx={sizes.outer - 10}
                  cy={-sizes.outer + 10}
                  r="6"
                  fill={
                    node.status === 'active' ? COLORS.statusActive :
                    node.status === 'warning' ? COLORS.statusWarning :
                    node.status === 'error' ? COLORS.statusError :
                    COLORS.statusInactive
                  }
                  stroke="#0a1929"
                  strokeWidth="2"
                  className="animate-pulse"
                />

                {/* Token symbol inside executor */}
                {isExecutor && node.status === 'active' && (
                  <>
                    <text
                      y={-8}
                      textAnchor="middle"
                      fill={node.color}
                      fontSize="14"
                      fontWeight="700"
                    >
                      {currentToken}
                    </text>
                    {/* Signal pulse ring */}
                    {hasRecentSignal && (
                      <circle
                        r="35"
                        fill="none"
                        stroke={COLORS.viperPurple}
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

                {/* Label */}
                <text
                  y={sizes.outer + 25}
                  textAnchor="middle"
                  fill="#ffffff"
                  fontSize="10"
                  fontWeight="700"
                  fontFamily="system-ui, -apple-system, sans-serif"
                  style={{ textTransform: 'uppercase', letterSpacing: '0.5px' }}
                >
                  {node.label}
                </text>

                {/* Sublabel (Latency) */}
                <text
                  y={sizes.outer + 38}
                  textAnchor="middle"
                  fill={node.color}
                  fontSize="9"
                  fontFamily="system-ui, -apple-system, sans-serif"
                  opacity="0.8"
                >
                  {node.sublabel}
                </text>
              </g>
            );
          })}
        </g>
      </svg>

      {/* Legend - Updated Colors */}
      <div className="absolute bottom-4 left-6 z-10 flex flex-wrap gap-4 text-xs">
        <div className="flex items-center gap-2 px-3 py-1.5 bg-[#0a1929]/80 border border-[#00ff88]/30 rounded-lg backdrop-blur-sm">
          <div className="w-3 h-3 rounded-full bg-[#10b981] shadow-lg shadow-[#10b981]/50 animate-pulse" />
          <span className="text-[#00ff88] font-medium">Active</span>
        </div>
        <div className="flex items-center gap-2 px-3 py-1.5 bg-[#0a1929]/80 border border-[#f59e0b]/30 rounded-lg backdrop-blur-sm">
          <div className="w-3 h-3 rounded-full bg-[#f59e0b] shadow-lg shadow-[#f59e0b]/50" />
          <span className="text-[#fbbf24] font-medium">Warning</span>
        </div>
        <div className="flex items-center gap-2 px-3 py-1.5 bg-[#0a1929]/80 border border-[#ef4444]/30 rounded-lg backdrop-blur-sm">
          <div className="w-3 h-3 rounded-full bg-[#ef4444] shadow-lg shadow-[#ef4444]/50" />
          <span className="text-[#f87171] font-medium">Error</span>
        </div>
        <div className="flex items-center gap-2 px-3 py-1.5 bg-[#0a1929]/80 border border-[#00d4ff]/30 rounded-lg backdrop-blur-sm">
          <div className="w-8 h-0.5 bg-gradient-to-r from-transparent via-[#00d4ff] to-transparent shadow-lg shadow-[#00d4ff]/50" />
          <span className="text-[#00d4ff] font-medium">Data Flow</span>
        </div>
      </div>

      {/* Tooltip */}
      {hoveredNode && (
        <div className="absolute bottom-16 right-6 z-20 px-4 py-3 bg-[#0a1929]/95 border border-cyan-500/40 rounded-lg shadow-2xl backdrop-blur-md min-w-[180px]">
          <p className="text-white font-bold text-sm tracking-wide">
            {NODES.find(n => n.id === hoveredNode)?.label}
          </p>
          <div className="mt-2 space-y-1">
            <p className="text-cyan-400 text-xs">
              Latency: <span className="font-mono font-semibold">{NODES.find(n => n.id === hoveredNode)?.latency}ms</span>
            </p>
            <p className="text-green-400 text-xs">
              Status: <span className="font-semibold">{NODES.find(n => n.id === hoveredNode)?.status.toUpperCase()}</span>
            </p>
            {NODES.find(n => n.id === hoveredNode)?.isCentral && (
              <p className="text-yellow-400 text-xs mt-2 pt-2 border-t border-cyan-900/30">
                ⭐ Central Node
              </p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
