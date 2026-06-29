'use client';

export type ConnectionStatus = 'connecting' | 'live' | 'stale' | 'down';

function resolveBaseWsUrl(): string {
  const override = process.env.NEXT_PUBLIC_WS_URL;
  if (override) return override;
  if (typeof window === 'undefined') return '';
  const proto = window.location.protocol === 'https:' ? 'wss' : 'ws';
  return `${proto}://${window.location.hostname}:8443/ws`;
}

async function fetchWsToken(): Promise<string | null> {
  try {
    const res = await fetch('/api/v1/auth/ws-token');
    if (!res.ok) return null;
    const data = await res.json();
    return data.token ?? null;
  } catch {
    return null;
  }
}

export class WebSocketClient {
  private ws: WebSocket | null = null;
  private baseUrl: string;
  private token: string | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private listeners = new Map<string, Set<(data: any) => void>>();
  private statusListeners = new Set<(status: ConnectionStatus) => void>();
  private retryCount = 0;
  private lastMessageAt = 0;
  private _status: ConnectionStatus = 'connecting';
  private active = true;
  private staleTimer: ReturnType<typeof setInterval> | null = null;

  static INITIAL_RETRY_MS = 1000;
  static MAX_RETRY_MS = 30000;
  static MAX_RETRIES = 20;
  static STALE_AFTER_MS = 20000;
  static STALE_CHECK_MS = 5000;

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl;
  }

  get status(): ConnectionStatus {
    return this._status;
  }

  get lastMessageTimestamp(): number {
    return this.lastMessageAt;
  }

  onStatusChange(cb: (status: ConnectionStatus) => void): () => void {
    this.statusListeners.add(cb);
    return () => this.statusListeners.delete(cb);
  }

  private resolveUrl(): string {
    if (this.token) {
      return `${this.baseUrl}?token=${encodeURIComponent(this.token)}`;
    }
    return this.baseUrl;
  }

  connect() {
    if (!this.active) return;
    if (!this.baseUrl) return;
    if (this.ws?.readyState === WebSocket.OPEN || this.ws?.readyState === WebSocket.CONNECTING) return;

    this.setStatus('connecting');
    try {
      this.ws = new WebSocket(this.resolveUrl());
      this.ws.onopen = () => {
        this.retryCount = 0;
        this.lastMessageAt = Date.now();
        this.setStatus('live');
        this.startStaleCheck();
      };
      this.ws.onmessage = (event) => {
        this.lastMessageAt = Date.now();
        if (this._status === 'stale') {
          this.setStatus('live');
        }
        try {
          const data = JSON.parse(event.data);
          this.dispatch(data);
        } catch {
          // ignore malformed messages
        }
      };
      this.ws.onclose = (event) => {
        this.stopStaleCheck();
        // If unauthorized, fetch a fresh token before reconnecting
        if (event.code === 4001 || event.code === 4401) {
          this.token = null;
          this.refreshToken();
        }
        if (!this.active) return;
        this.scheduleReconnect();
      };
      this.ws.onerror = () => {
        this.ws?.close();
      };
    } catch {
      this.scheduleReconnect();
    }
  }

  async refreshToken(): Promise<void> {
    this.token = await fetchWsToken();
  }

  private dispatch(data: Record<string, unknown>) {
    if (data.signal) {
      this.emit('market_signal', data);
    }
    if (data.decision) {
      this.emit('decision', data);
    }
    this.emit('message', data);
  }

  private startStaleCheck() {
    this.stopStaleCheck();
    this.staleTimer = setInterval(() => {
      if (this._status === 'live' && Date.now() - this.lastMessageAt > WebSocketClient.STALE_AFTER_MS) {
        this.setStatus('stale');
      }
    }, WebSocketClient.STALE_CHECK_MS);
  }

  private stopStaleCheck() {
    if (this.staleTimer) {
      clearInterval(this.staleTimer);
      this.staleTimer = null;
    }
  }

  private scheduleReconnect() {
    if (!this.active) return;
    this.setStatus('down');
    this.retryCount++;
    if (this.retryCount > WebSocketClient.MAX_RETRIES) return;

    const delay = Math.min(
      WebSocketClient.INITIAL_RETRY_MS * (1 << (this.retryCount - 1)),
      WebSocketClient.MAX_RETRY_MS
    );
    const jitter = delay * (0.5 + Math.random() * 0.5);
    this.reconnectTimer = setTimeout(() => this.connect(), jitter);
  }

  on(event: string, callback: (data: any) => void): () => void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(callback);
    return () => this.off(event, callback);
  }

  off(event: string, callback: (data: any) => void) {
    this.listeners.get(event)?.delete(callback);
  }

  private emit(event: string, data: any) {
    this.listeners.get(event)?.forEach((cb) => cb(data));
  }

  private setStatus(status: ConnectionStatus) {
    this._status = status;
    this.statusListeners.forEach((cb) => cb(status));
  }

  disconnect() {
    this.active = false;
    this.stopStaleCheck();
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.setStatus('down');
  }
}

let wsClient: WebSocketClient | null = null;
let tokenPromise: Promise<void> | null = null;

export async function getWebSocketClient(): Promise<WebSocketClient> {
  if (!wsClient) {
    wsClient = new WebSocketClient(resolveBaseWsUrl());
    tokenPromise = wsClient.refreshToken();
  }
  // Ensure token is fetched before returning on first call
  if (tokenPromise) {
    await tokenPromise;
    tokenPromise = null;
  }
  return wsClient;
}
