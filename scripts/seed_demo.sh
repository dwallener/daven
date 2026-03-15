#!/bin/zsh
set -euo pipefail

WORKFLOW_URL="${VITE_WORKFLOW_API_URL:-http://127.0.0.1:3003}"
INGEST_URL="${VITE_INGEST_API_URL:-http://127.0.0.1:3002}"
ASSET_URL="${VITE_ASSET_API_URL:-http://127.0.0.1:3004}"
RECOMMENDATION_URL="${VITE_RECOMMENDATION_API_URL:-http://127.0.0.1:3005}"
PLANNING_URL="${VITE_PLANNING_API_URL:-http://127.0.0.1:3006}"
EXECUTION_URL="${VITE_EXECUTION_API_URL:-http://127.0.0.1:3007}"
ASSESSMENT_URL="${VITE_ASSESSMENT_API_URL:-http://127.0.0.1:3008}"

DEMO_OPERATOR="${DEMO_OPERATOR:-demo-operator}"

extract_id() {
  local payload="$1"
  local id
  id="$(printf '%s' "$payload" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')"
  if [[ -z "$id" ]]; then
    printf 'failed to extract id from payload:\n%s\n' "$payload" >&2
    exit 1
  fi
  printf '%s' "$id"
}

create_detection() {
  local title="$1"
  local source_id="$2"
  local external_ref="$3"
  local classification="$4"
  local confidence="$5"
  local lng="$6"
  local lat="$7"
  local payload
  payload="$(curl -fsS -X POST "$INGEST_URL/ingest/detections" \
    -H 'content-type: application/json' \
    -d "{\"source_type\":\"cv_fmv\",\"source_id\":\"$source_id\",\"external_ref\":\"$external_ref\",\"location\":{\"type\":\"Point\",\"coordinates\":[$lng,$lat]},\"classification\":\"$classification\",\"confidence\":$confidence}")"
  local detection_id
  detection_id="$(extract_id "$payload")"
  printf '%s|%s\n' "$title" "$detection_id"
}

nominate_target() {
  local detection_id="$1"
  local title="$2"
  local labels="$3"
  local payload
  payload="$(curl -fsS -X POST "$WORKFLOW_URL/detections/$detection_id/nominate" \
    -H 'content-type: application/json' \
    -d "{\"actor\":\"$DEMO_OPERATOR\",\"labels\":$labels}")"
  local target_id
  target_id="$(extract_id "$payload")"
  printf '%s|%s\n' "$title" "$target_id"
}

create_asset() {
  local callsign="$1"
  local platform_type="$2"
  local domain="$3"
  local capabilities="$4"
  local lng="$5"
  local lat="$6"
  local payload
  payload="$(curl -fsS -X POST "$ASSET_URL/assets" \
    -H 'content-type: application/json' \
    -d "{\"callsign\":\"$callsign\",\"platform_type\":\"$platform_type\",\"domain\":\"$domain\",\"location\":{\"type\":\"Point\",\"coordinates\":[$lng,$lat]},\"capabilities\":$capabilities}")"
  local asset_id
  asset_id="$(extract_id "$payload")"
  printf '%s|%s\n' "$callsign" "$asset_id"
}

refresh_recommendations() {
  local target_id="$1"
  curl -fsS "$RECOMMENDATION_URL/recommendations/targets/$target_id" >/dev/null
}

propose_task() {
  local target_id="$1"
  local asset_ids="$2"
  local task_type="$3"
  local effect_type="$4"
  local payload
  payload="$(curl -fsS -X POST "$PLANNING_URL/tasks/propose" \
    -H 'content-type: application/json' \
    -d "{\"target_id\":\"$target_id\",\"asset_ids\":$asset_ids,\"task_type\":\"$task_type\",\"effect_type\":\"$effect_type\",\"created_by\":\"$DEMO_OPERATOR\"}")"
  extract_id "$payload"
}

approve_task() {
  local task_id="$1"
  curl -fsS -X POST "$PLANNING_URL/tasks/$task_id/approve" \
    -H 'content-type: application/json' \
    -d "{\"actor\":\"$DEMO_OPERATOR\"}" >/dev/null
}

dispatch_task() {
  local task_id="$1"
  local notes="$2"
  curl -fsS -X POST "$EXECUTION_URL/tasks/$task_id/dispatch" \
    -H 'content-type: application/json' \
    -d "{\"actor\":\"$DEMO_OPERATOR\",\"notes\":\"$notes\"}" >/dev/null
}

complete_task() {
  local task_id="$1"
  local notes="$2"
  curl -fsS -X POST "$EXECUTION_URL/tasks/$task_id/complete" \
    -H 'content-type: application/json' \
    -d "{\"actor\":\"$DEMO_OPERATOR\",\"notes\":\"$notes\"}" >/dev/null
}

assess_task() {
  local task_id="$1"
  local result="$2"
  local confidence="$3"
  local notes="$4"
  local media_refs="$5"
  curl -fsS -X POST "$ASSESSMENT_URL/tasks/$task_id/assess" \
    -H 'content-type: application/json' \
    -d "{\"actor\":\"$DEMO_OPERATOR\",\"result\":\"$result\",\"confidence\":$confidence,\"notes\":\"$notes\",\"media_refs\":$media_refs}" >/dev/null
}

north_ramp_detection="$(create_detection "North ramp vehicle" "feed_alpha" "frame_455:obj_ramp" "vehicle" "0.91" "50.3528" "29.2554")"
harbor_detection="$(create_detection "Harbor contact" "feed_bravo" "frame_812:obj_harbor" "vessel" "0.88" "50.3407" "29.2194")"
tank_farm_detection="$(create_detection "Tank farm mover" "feed_charlie" "frame_990:obj_tankfarm" "vehicle" "0.94" "50.3231" "29.2128")"
southern_detection="$(create_detection "Southern depot stack" "feed_delta" "frame_121:obj_depot" "equipment" "0.86" "50.3179" "29.2034")"
runway_detection="$(create_detection "Runway support truck" "feed_echo" "frame_612:obj_runway" "vehicle" "0.89" "50.3665" "29.2674")"

north_ramp_target="$(nominate_target "${north_ramp_detection#*|}" "North ramp vehicle" '["dynamic","kharg","ramp"]')"
harbor_target="$(nominate_target "${harbor_detection#*|}" "Harbor contact" '["dynamic","kharg","harbor"]')"
tank_farm_target="$(nominate_target "${tank_farm_detection#*|}" "Tank farm mover" '["dynamic","kharg","energy"]')"
southern_target="$(nominate_target "${southern_detection#*|}" "Southern depot stack" '["dynamic","kharg","depot"]')"
runway_target="$(nominate_target "${runway_detection#*|}" "Runway support truck" '["dynamic","kharg","runway"]')"

stryker_north="$(create_asset "StrykerNorth" "stryker" "land" '["observe","strike"]' "50.3478" "29.2508")"
observer_east="$(create_asset "ObserverEast" "uav" "air" '["observe"]' "50.3594" "29.2417")"
sentinel_harbor="$(create_asset "SentinelHarbor" "usv" "maritime" '["observe","ew"]' "50.3464" "29.2217")"
stryker_south="$(create_asset "StrykerSouth" "stryker" "land" '["observe","strike"]' "50.3258" "29.2058")"
watchtower_west="$(create_asset "WatchtowerWest" "sensor" "land" '["observe"]' "50.3008" "29.2304")"

for target in \
  "${north_ramp_target#*|}" \
  "${harbor_target#*|}" \
  "${tank_farm_target#*|}" \
  "${southern_target#*|}" \
  "${runway_target#*|}"
do
  refresh_recommendations "$target"
done

runway_task="$(propose_task "${runway_target#*|}" "[\"${observer_east#*|}\"]" "OBSERVE" "collection")"
approve_task "$runway_task"

tank_farm_task="$(propose_task "${tank_farm_target#*|}" "[\"${stryker_north#*|}\",\"${observer_east#*|}\"]" "SMACK" "kinetic")"
approve_task "$tank_farm_task"
dispatch_task "$tank_farm_task" "moving north package into the tank farm engagement box"

harbor_task="$(propose_task "${harbor_target#*|}" "[\"${sentinel_harbor#*|}\"]" "TRACK" "monitor")"
approve_task "$harbor_task"
dispatch_task "$harbor_task" "maintain harbor surveillance on outbound vessel"
complete_task "$harbor_task" "track complete with sustained harbor coverage"

southern_task="$(propose_task "${southern_target#*|}" "[\"${stryker_south#*|}\",\"${watchtower_west#*|}\"]" "SMACK" "kinetic")"
approve_task "$southern_task"
dispatch_task "$southern_task" "south package committed on depot stack"
complete_task "$southern_task" "follow-on strike window complete"
assess_task "$southern_task" "DESTROYED" "0.92" "secondary effects visible over the southern depot" '["uav_clip_021.mp4","sar_pass_011.tif"]'

cat <<EOF
Seeded Kharg demo scenario:
  detections:
    ${north_ramp_detection#*|}
    ${harbor_detection#*|}
    ${tank_farm_detection#*|}
    ${southern_detection#*|}
    ${runway_detection#*|}

  targets:
    deliberate:      ${north_ramp_target#*|}
    paired:          ${runway_target#*|}
    in_execution:    ${tank_farm_target#*|}
    pending_bda:     ${harbor_target#*|}
    complete:        ${southern_target#*|}

  assets:
    ${stryker_north#*|}
    ${observer_east#*|}
    ${sentinel_harbor#*|}
    ${stryker_south#*|}
    ${watchtower_west#*|}

  tasks:
    paired:          $runway_task
    in_execution:    $tank_farm_task
    pending_bda:     $harbor_task
    complete:        $southern_task

Next in the UI:
  1. Zoom into the north ramp, harbor, and southern depot clusters
  2. Open the paired or in-execution target from the board
  3. Review the recommendation stack and task history
  4. Continue the remaining deliberate or pending-BDA targets live
EOF
