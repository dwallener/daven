#!/bin/zsh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="$ROOT_DIR/.demo/logs"
DB_URL="${DATABASE_URL:-postgresql:///daven}"

PACKAGES=(
  "ingest-service"
  "workflow-service"
  "asset-service"
  "recommendation-service"
  "planning-service"
  "execution-adapter"
  "assessment-service"
)

mkdir -p "$LOG_DIR"

start_service() {
  local name="$1"
  local port="$2"
  local binary="$3"
  local log_file="$LOG_DIR/${name}.log"

  if lsof -ti "tcp:$port" >/dev/null 2>&1; then
    echo "$name not started because port $port is already in use"
    return
  fi

  echo "Starting $name on :$port"
  (
    cd "$ROOT_DIR"
    nohup env APP_PORT="$port" DATABASE_URL="$DB_URL" "$ROOT_DIR/target/debug/$binary" \
      >"$log_file" 2>&1 &
  ) &
}

echo "Building backend services"
(
  cd "$ROOT_DIR"
  cargo build \
    -p "${PACKAGES[1]}" \
    -p "${PACKAGES[2]}" \
    -p "${PACKAGES[3]}" \
    -p "${PACKAGES[4]}" \
    -p "${PACKAGES[5]}" \
    -p "${PACKAGES[6]}" \
    -p "${PACKAGES[7]}"
)

start_service "ingest-service" 3002 "ingest-service"
start_service "workflow-service" 3003 "workflow-service"
start_service "asset-service" 3004 "asset-service"
start_service "recommendation-service" 3005 "recommendation-service"
start_service "planning-service" 3006 "planning-service"
start_service "execution-adapter" 3007 "execution-adapter"
start_service "assessment-service" 3008 "assessment-service"

cat <<EOF

Backend services started.
Logs: $LOG_DIR

Start the frontend separately:
  cd "$ROOT_DIR/apps/frontend"
  npm run dev
EOF
