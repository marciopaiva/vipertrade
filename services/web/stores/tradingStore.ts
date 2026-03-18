import { create } from 'zustand';

interface TradingState {
  // Market Data
  marketSignals: any[];
  setMarketSignals: (signals: any[]) => void;
  
  // Positions
  positions: any[];
  setPositions: (positions: any[]) => void;
  
  // Trades
  trades: any[];
  setTrades: (trades: any[]) => void;
  
  // Wallet
  wallet: any | null;
  setWallet: (wallet: any | null) => void;
  
  // Services Health
  services: any[];
  setServices: (services: any[]) => void;
  
  // UI State
  selectedToken: string | null;
  setSelectedToken: (token: string | null) => void;
}

export const useTradingStore = create<TradingState>((set) => ({
  // Market Data
  marketSignals: [],
  setMarketSignals: (signals) => set({ marketSignals: signals }),
  
  // Positions
  positions: [],
  setPositions: (positions) => set({ positions: positions }),
  
  // Trades
  trades: [],
  setTrades: (trades) => set({ trades: trades }),
  
  // Wallet
  wallet: null,
  setWallet: (wallet) => set({ wallet: wallet }),
  
  // Services Health
  services: [],
  setServices: (services) => set({ services: services }),
  
  // UI State
  selectedToken: null,
  setSelectedToken: (token) => set({ selectedToken: token }),
}));
