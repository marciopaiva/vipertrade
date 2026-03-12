FROM python:3.12-slim-bookworm

ENV LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH

RUN apt-get update && apt-get install -y --no-install-recommends \
  ca-certificates \
  libssl3 \
  curl \
  libpython3.11 \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app
