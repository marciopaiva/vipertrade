ARG NODE_VERSION=20

# Stage 1: Install all dependencies (including dev for build)
FROM node:${NODE_VERSION} AS deps
WORKDIR /app
COPY services/web/package.json services/web/yarn.lock ./
RUN corepack enable && yarn install --frozen-lockfile

# Stage 2: Build Next.js (NEXT_PUBLIC_* vars must be present at build time)
FROM node:${NODE_VERSION} AS builder
ARG NEXT_PUBLIC_API_URL
ARG NEXT_PUBLIC_WS_URL
ARG NEXT_PUBLIC_TRADING_MODE
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL
ENV NEXT_PUBLIC_WS_URL=$NEXT_PUBLIC_WS_URL
ENV NEXT_PUBLIC_TRADING_MODE=$NEXT_PUBLIC_TRADING_MODE
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY services/web/ .
RUN corepack enable && yarn build

# Stage 3: Production dependencies only (smaller runtime)
FROM node:${NODE_VERSION} AS prod-deps
WORKDIR /app
COPY services/web/package.json services/web/yarn.lock ./
RUN corepack enable && yarn install --production --frozen-lockfile

# Stage 4: Runner
FROM node:${NODE_VERSION} AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV PORT=3000
ENV HOSTNAME="0.0.0.0"
ENV VIPERTRADE_SECURE_COOKIE=false
RUN apt-get update && apt-get install -y --no-install-recommends netcat-openbsd && \
    rm -rf /var/lib/apt/lists/* && \
    addgroup --system --gid 1001 nodejs && \
    adduser --system --uid 1001 nextjs
COPY --from=prod-deps /app/node_modules ./node_modules
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./public/_next/static
COPY --from=builder /app/public ./public
COPY --chown=nextjs:nodejs scripts/docker/wait-for-deps.sh /usr/local/bin/wait-for-deps.sh
RUN chmod +x /usr/local/bin/wait-for-deps.sh
RUN mkdir -p /app/.next/cache && chown -R nextjs:nodejs /app/.next
USER nextjs
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
  CMD wget --quiet --tries=1 --spider http://localhost:3000/api/health || exit 1
CMD ["wait-for-deps.sh", "postgres:5432", "redis:6379", "api:8080", "ai-analyst:8087", "--", "node", "server.js"]
