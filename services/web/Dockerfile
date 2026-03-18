# ✅ ViperTrade Web - Optimized Multi-Stage Dockerfile
# Security + Performance + Minimal Image Size

ARG NODE_VERSION=20

# Stage 1: Dependencies (production only)
FROM node:${NODE_VERSION}-alpine AS deps
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production --frozen-lockfile

# Stage 2: Builder
FROM node:${NODE_VERSION}-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

# Stage 3: Runner (minimal production image)
FROM node:${NODE_VERSION}-alpine AS runner
WORKDIR /app

# Environment
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

# Security: Create non-root user
RUN addgroup --system --gid 1001 nodejs && \
    adduser --system --uid 1001 nextjs

# Copy static files first
COPY --from=builder /app/public ./public
COPY --from=builder /app/.next/static ./public/_next/static

# Copy standalone output
COPY --from=builder /app/.next/standalone ./
COPY --from=deps /app/node_modules ./node_modules

# Set ownership
RUN chown -R nextjs:nodejs /app

# Switch to non-root user
USER nextjs

EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD wget --quiet --tries=1 --spider http://localhost:3000/api/health || exit 1

CMD ["node", "server.js"]
