# ViperTrade Web Dashboard

Modern, real-time trading dashboard for ViperTrade Lead Trader Bot.

## 🚀 Features

- **Real-time Data**: WebSocket connections for live market data
- **Responsive Design**: Mobile-first with Tailwind CSS
- **Dark Theme**: Optimized for trading environments
- **Error Boundaries**: Graceful error handling
- **Loading States**: Skeleton screens and spinners
- **Type Safe**: Full TypeScript support

## 📁 Structure

```
services/web/
├── app/                    # Next.js App Router
│   ├── (dashboard)/        # Dashboard route group
│   ├── api/                # API routes
│   ├── layout.tsx          # Root layout
│   ├── page.tsx            # Main dashboard
│   ├── loading.tsx         # Global loading
│   ├── error.tsx           # Error boundary
│   └── not-found.tsx       # 404 page
├── components/
│   ├── ui/                 # shadcn/ui components
│   ├── dashboard/          # Dashboard-specific components
│   └── layout/             # Header, Footer
├── hooks/                  # Custom React hooks
├── lib/                    # Utilities, API clients
├── stores/                 # Zustand state management
└── types/                  # TypeScript types
```

## 🛠️ Development

### Prerequisites

- Node.js 20+
- npm or yarn
- Docker (for local backend)

### Setup

```bash
# Install dependencies
npm install

# Copy environment variables
cp .env.example .env.local

# Start development server
npm run dev
```

### Available Scripts

```bash
# Development
npm run dev              # Start dev server
npm run build            # Production build
npm run start            # Start production server

# Code Quality
npm run lint             # ESLint check
npm run lint:fix         # Auto-fix lint errors
npm run type-check       # TypeScript check
npm run format           # Prettier format

# Testing
npm run test             # Vitest watch mode
npm run test:run         # Run tests once
npm run test:coverage    # Tests with coverage

# Docker
npm run docker:build     # Build Docker image
npm run docker:dev       # Start with docker-compose
npm run docker:logs      # View logs
npm run docker:stop      # Stop containers
```

## 🎨 Components

### UI Components (shadcn/ui)

- Button
- Card
- Badge
- ...and more

### Dashboard Components

- ArchitectureFlowSVG
- WalletOverview
- PositionTable
- TradeHistory
- DecisionMatrix

## 🔌 API Integration

### Environment Variables

```bash
# .env.local
NEXT_PUBLIC_API_URL=http://localhost:8080
NEXT_PUBLIC_WS_URL=ws://localhost:8080/ws
NEXT_PUBLIC_TRADING_MODE=paper
```

### Custom Hooks

```typescript
import { useMarketData } from '@/hooks/useMarketData';
import { usePositions } from '@/hooks/usePositions';
import { useTrades } from '@/hooks/useTrades';
import { useServiceHealth } from '@/hooks/useServiceHealth';

function Dashboard() {
  const { marketSignals } = useMarketData();
  const { positions } = usePositions();
  const { trades } = useTrades();
  const { services } = useServiceHealth();
  
  // ...
}
```

### State Management (Zustand)

```typescript
import { useTradingStore } from '@/stores/tradingStore';

function Component() {
  const { positions, setPositions } = useTradingStore();
  
  // ...
}
```

## 🐳 Docker

### Build

```bash
npm run docker:build
# or
docker build -t vipertrade-web:latest .
```

### Run

```bash
npm run docker:run
# or
docker run -p 3000:3000 --env-file .env vipertrade-web:latest
```

### Development

```bash
npm run docker:dev
# or
docker-compose up -d
```

## 📊 Routes

| Route | Description |
|-------|-------------|
| `/` | Main dashboard |
| `/trades` | Trade history |
| `/positions` | Open positions |
| `/settings` | Settings |
| `/api/health` | Health check |

## 🎯 Performance

### Optimizations

- **Next.js Standalone**: Minimal Docker image (~200MB)
- **SWC Minifier**: Fast builds (Rust-based)
- **Image Optimization**: WebP + AVIF formats
- **Code Splitting**: Automatic by Next.js
- **Lazy Loading**: Components loaded on demand

### Bundle Analysis

```bash
npm run analyze
```

## 🔒 Security

- **Non-root user**: Container runs as `nextjs:1001`
- **Security Headers**: HSTS, XSS, Frame-Options
- **Environment Variables**: Sensitive data in `.env.local`
- **TypeScript Strict Mode**: Type safety enabled

## 🧪 Testing

```bash
# Watch mode
npm run test

# With UI
npm run test:ui

# Coverage
npm run test:coverage
```

## 📝 Code Style

### ESLint + Prettier

```bash
# Check
npm run lint
npm run format:check

# Fix
npm run lint:fix
npm run format
```

## 🤝 Contributing

1. Create feature branch from `feature/web-v2`
2. Make changes
3. Run tests and lint
4. Create PR to `feature/web-v2`
5. Review and merge

## 📄 License

MIT

## 🔗 Links

- [ViperTrade Main Repo](https://github.com/marciopaiva/vipertrade)
- [Next.js Docs](https://nextjs.org/docs)
- [Tailwind CSS](https://tailwindcss.com)
- [shadcn/ui](https://ui.shadcn.com)
- [Zustand](https://zustand-demo.pmnd.rs)
