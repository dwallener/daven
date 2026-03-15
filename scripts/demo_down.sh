#!/bin/zsh
set -euo pipefail

for port in 3002 3003 3004 3005 3006 3007 3008; do
  pid="$(lsof -ti "tcp:$port" || true)"
  if [[ -n "$pid" ]]; then
    echo "Stopping process on port $port ($pid)"
    kill "$pid"
  fi
done

echo "Demo services stopped."
