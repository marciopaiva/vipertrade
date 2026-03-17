ARG NODE_VERSION=20
ARG WEB_BASE_IMAGE=node:${NODE_VERSION}-bookworm-slim
FROM ${WEB_BASE_IMAGE}

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends wget && rm -rf /var/lib/apt/lists/*

COPY package*.json ./
RUN npm ci

COPY . .

RUN npm run build

CMD ["npm", "run", "start"]
