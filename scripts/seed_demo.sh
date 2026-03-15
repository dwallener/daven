#!/bin/zsh
set -euo pipefail

WORKFLOW_URL="${VITE_WORKFLOW_API_URL:-http://127.0.0.1:3003}"
INGEST_URL="${VITE_INGEST_API_URL:-http://127.0.0.1:3002}"
ASSET_URL="${VITE_ASSET_API_URL:-http://127.0.0.1:3004}"
RECOMMENDATION_URL="${VITE_RECOMMENDATION_API_URL:-http://127.0.0.1:3005}"
PLANNING_URL="${VITE_PLANNING_API_URL:-http://127.0.0.1:3006}"
EXECUTION_URL="${VITE_EXECUTION_API_URL:-http://127.0.0.1:3007}"
ASSESSMENT_URL="${VITE_ASSESSMENT_API_URL:-http://127.0.0.1:3008}"

detection_json="$(curl -s -X POST "$INGEST_URL/ingest/detections" \
  -H 'content-type: application/json' \
  -d '{"source_type":"cv_fmv","source_id":"feed_alpha","external_ref":"frame_455:obj_demo","location":{"type":"Point","coordinates":[-122.91,49.21]},"classification":"vehicle","confidence":0.91}')"
detection_id="$(printf '%s' "$detection_json" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')"

target_json="$(curl -s -X POST "$WORKFLOW_URL/detections/$detection_id/nominate" \
  -H 'content-type: application/json' \
  -d '{"actor":"demo-operator","labels":["dynamic","demo"]}')"
target_id="$(printf '%s' "$target_json" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')"

asset_json="$(curl -s -X POST "$ASSET_URL/assets" \
  -H 'content-type: application/json' \
  -d '{"callsign":"StrykerDemo","platform_type":"stryker","domain":"land","location":{"type":"Point","coordinates":[-122.89,49.19]},"capabilities":["observe","strike"]}')"
asset_id="$(printf '%s' "$asset_json" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')"

curl -s "$RECOMMENDATION_URL/recommendations/targets/$target_id" >/dev/null

task_json="$(curl -s -X POST "$PLANNING_URL/tasks/propose" \
  -H 'content-type: application/json' \
  -d "{\"target_id\":\"$target_id\",\"asset_ids\":[\"$asset_id\"],\"task_type\":\"SMACK\",\"effect_type\":\"kinetic\",\"created_by\":\"demo-operator\"}")"
task_id="$(printf '%s' "$task_json" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')"

curl -s -X POST "$PLANNING_URL/tasks/$task_id/approve" \
  -H 'content-type: application/json' \
  -d '{"actor":"demo-operator"}' >/dev/null

cat <<EOF
Seeded demo entities:
  detection: $detection_id
  target:    $target_id
  asset:     $asset_id
  task:      $task_id

Next in the UI:
  1. Open the target
  2. Dispatch task
  3. Complete task
  4. Submit assessment
EOF
