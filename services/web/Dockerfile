# ViperTrade Web - Optimized Multi-Stage Dockerfile
# Security + Performance + Minimal Image Size

ARG NODE_VERSION=20

# Stage 1: Dependencies (production only)
FROM node:${NODE_VERSION} AS deps
WORKDIR /app
COPY package.json yarn.lock ./
RUN corepack enable && yarn install --production --frozen-lockfile

# Stage 2: Runner (production image)
FROM node:${NODE_VERSION} AS runner
WORKDIR /app

# Environment
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV PORT=3000
ENV HOSTNAME="0.0.0.0"
# Default secure cookie flag: false for dev; override via deployment env if needed
ENV VIPERTRADE_SECURE_COOKIE=false

# Security: Create non-root user and install netcat
RUN apt-get update && apt-get install -y --no-install-recommends netcat-openbsd && \
    rm -rf /var/lib/apt/lists/* && \
    addgroup --system --gid 1001 nodejs && \
    adduser --system --uid 1001 nextjs

# Copy production node_modules from deps stage
COPY --from=deps /app/node_modules ./node_modules

# Copy public assets
COPY public ./public

# Copy standalone build (must run `yarn build` on host before building image)
COPY --chown=nextjs:nodejs .next/standalone ./

# Copy .next/static for client-side assets
COPY --chown=nextjs:nodejs .next/static ./public/_next/static

# Copy wait-for-deps script
COPY --chown=nextjs:nodejs docker/wait-for-deps.sh /usr/local/bin/wait-for-deps.sh
RUN chmod +x /usr/local/bin/wait-for-deps.sh

# Switch to non-root user
USER nextjs

EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
  CMD wget --quiet --tries=1 --spider http://localhost:3000/api/health || exit 1

CMD ["wait-for-deps.sh", "postgres:5432", "redis:6379", "api:8080", "ai-analyst:8087", "--", "node", "server.js"]
