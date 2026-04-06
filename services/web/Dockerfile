# ✅ ViperTrade Web - Optimized Multi-Stage Dockerfile
# Security + Performance + Minimal Image Size

ARG NODE_VERSION=20

# Stage 1: Dependencies (production only)
FROM node:${NODE_VERSION}-alpine AS deps
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production --legacy-peer-deps --frozen-lockfile

# Stage 2: Builder
FROM node:${NODE_VERSION}-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci --legacy-peer-deps
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
RUN apk add --no-cache netcat-openbsd && \
    addgroup --system --gid 1001 nodejs && \
    adduser --system --uid 1001 nextjs

# Copy public directory first
COPY --from=builder /app/public ./public

# Copy standalone server output
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./

# Copy .next/static for client-side assets
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./public/_next/static

# Copy node_modules from deps stage
COPY --from=deps /app/node_modules ./node_modules
COPY --chown=nextjs:nodejs docker/wait-for-deps.sh /usr/local/bin/wait-for-deps.sh
RUN chmod +x /usr/local/bin/wait-for-deps.sh

# Install sharp for Next.js image optimization
RUN npm install --legacy-peer-deps --arch=x64 --platform=linux sharp

# Set ownership for remaining files
RUN chown -R nextjs:nodejs /app

# Switch to non-root user
USER nextjs

EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
  CMD wget --quiet --tries=1 --spider http://localhost:3000/api/health || exit 1

CMD ["wait-for-deps.sh", "postgres:5432", "redis:6379", "api:8080", "ai-analyst:8087", "--", "node", "server.js"]
