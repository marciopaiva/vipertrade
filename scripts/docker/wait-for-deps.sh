#!/bin/sh
set -eu

TIMEOUT_SECONDS="${WAIT_FOR_DEPS_TIMEOUT_SECONDS:-90}"
SLEEP_SECONDS="${WAIT_FOR_DEPS_SLEEP_SECONDS:-2}"

if [ "$#" -lt 2 ]; then
  echo "usage: wait-for-deps.sh host:port [host:port ...] -- command [args...]" >&2
  exit 2
fi

DEPS=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--" ]; then
    shift
    break
  fi
  DEPS="$DEPS $1"
  shift
done

if [ "$#" -eq 0 ]; then
  echo "wait-for-deps.sh: missing command after --" >&2
  exit 2
fi

wait_for_dep() {
  dep="$1"
  host="${dep%:*}"
  port="${dep##*:}"
  start_ts="$(date +%s)"

  echo "wait-for-deps: waiting for ${host}:${port}"
  while ! nc -z "$host" "$port" >/dev/null 2>&1; do
    now_ts="$(date +%s)"
    elapsed="$((now_ts - start_ts))"
    if [ "$elapsed" -ge "$TIMEOUT_SECONDS" ]; then
      echo "wait-for-deps: timeout while waiting for ${host}:${port}" >&2
      exit 1
    fi
    sleep "$SLEEP_SECONDS"
  done
  echo "wait-for-deps: ${host}:${port} is reachable"
}

for dep in $DEPS; do
  wait_for_dep "$dep"
done

exec "$@"
