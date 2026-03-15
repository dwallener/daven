# Daven

Daven is an MVP scaffold for an integrated detection-to-action platform.

Current implemented scope:

- Native PostgreSQL-backed workflow service
- Native PostgreSQL-backed ingest service
- Native PostgreSQL-backed asset service
- Native PostgreSQL-backed recommendation service
- Native PostgreSQL-backed planning service
- React + TypeScript workflow board that reads the live APIs

## Workspace layout

- `apps/api-gateway`: placeholder API gateway crate
- `apps/frontend`: React + TypeScript board UI
- `libs/config`: shared runtime configuration
- `libs/domain-models`: canonical domain types
- `libs/event-contracts`: shared event payloads and envelope
- `services/ingest-service`: detection ingest and lookup
- `services/workflow-service`: targets, transitions, boards, nomination
- `services/asset-service`: asset registry and telemetry updates
- `services/recommendation-service`: target-to-asset ranking
- `services/planning-service`: task proposal and approval flow
- `infra/db/migrations`: bootstrap schema

## Prerequisites

- Rust toolchain with `cargo`
- Node.js + npm
- PostgreSQL running locally

This repo is currently optimized for native local development on macOS.

## Database setup

Create the local database once:

```zsh
createdb daven
```

The services use:

```zsh
DATABASE_URL='postgresql:///daven'
```

That uses your local PostgreSQL socket and current macOS username.

## Run the backend

Open one terminal per service from the repo root.

Workflow service on `3003`:

```zsh
APP_PORT=3003 DATABASE_URL='postgresql:///daven' cargo run -p workflow-service
```

Ingest service on `3002`:

```zsh
APP_PORT=3002 DATABASE_URL='postgresql:///daven' cargo run -p ingest-service
```

Asset service on `3004`:

```zsh
APP_PORT=3004 DATABASE_URL='postgresql:///daven' cargo run -p asset-service
```

Recommendation service on `3005`:

```zsh
APP_PORT=3005 DATABASE_URL='postgresql:///daven' cargo run -p recommendation-service
```

Planning service on `3006`:

```zsh
APP_PORT=3006 DATABASE_URL='postgresql:///daven' cargo run -p planning-service
```

If a service says `Address already in use`, another copy is already running on that port.

## Run the frontend

From the repo root:

```zsh
cd apps/frontend
npm install
npm run dev
```

By default the frontend expects:

- workflow API: `http://127.0.0.1:3003`
- asset API: `http://127.0.0.1:3004`
- recommendation API: `http://127.0.0.1:3005`
- planning API: `http://127.0.0.1:3006`

You can override those with Vite env vars:

```zsh
VITE_WORKFLOW_API_URL=http://127.0.0.1:3003
VITE_ASSET_API_URL=http://127.0.0.1:3004
VITE_RECOMMENDATION_API_URL=http://127.0.0.1:3005
VITE_PLANNING_API_URL=http://127.0.0.1:3006
```

Example:

```zsh
cd apps/frontend
VITE_WORKFLOW_API_URL=http://127.0.0.1:3003 \
VITE_ASSET_API_URL=http://127.0.0.1:3004 \
VITE_RECOMMENDATION_API_URL=http://127.0.0.1:3005 \
VITE_PLANNING_API_URL=http://127.0.0.1:3006 \
npm run dev
```

## First end-to-end test

1. Start the five backend services.
2. Create a detection:

```zsh
curl -s -X POST http://127.0.0.1:3002/ingest/detections \
  -H 'content-type: application/json' \
  -d '{"source_type":"cv_fmv","source_id":"feed_alpha","external_ref":"frame_455:obj_12","location":{"type":"Point","coordinates":[-122.91,49.21]},"classification":"vehicle","confidence":0.87}'
```

3. Nominate that detection in workflow:

```zsh
curl -s -X POST http://127.0.0.1:3003/detections/DETECTION_ID/nominate \
  -H 'content-type: application/json' \
  -d '{"actor":"damir00","labels":["dynamic"]}'
```

4. Create assets:

```zsh
curl -s -X POST http://127.0.0.1:3004/assets \
  -H 'content-type: application/json' \
  -d '{"callsign":"Stryker1","platform_type":"stryker","domain":"land","location":{"type":"Point","coordinates":[-122.89,49.19]},"capabilities":["observe","strike"]}'
```

5. Request recommendations:

```zsh
curl -s http://127.0.0.1:3005/recommendations/targets/TARGET_ID
```

6. Propose a task:

```zsh
curl -s -X POST http://127.0.0.1:3006/tasks/propose \
  -H 'content-type: application/json' \
  -d '{"target_id":"TARGET_ID","asset_ids":["ASSET_ID"],"task_type":"SMACK","effect_type":"kinetic","created_by":"damir00"}'
```

7. Approve it:

```zsh
curl -s -X POST http://127.0.0.1:3006/tasks/TASK_ID/approve \
  -H 'content-type: application/json' \
  -d '{"actor":"damir00"}'
```

8. Open the frontend and inspect the live board.

## Implemented backend slices

### Ingest

- `POST /ingest/detections`
- `GET /detections/{id}`

### Workflow

- `GET /health`
- `POST /targets`
- `GET /targets/{id}`
- `POST /targets/{id}/transition`
- `GET /boards`
- `GET /boards/{id}`
- `GET /boards/{id}/targets`
- `POST /detections/{id}/nominate`

### Assets

- `GET /assets`
- `POST /assets`
- `GET /assets/{id}`
- `PATCH /assets/{id}`
- `POST /assets/{id}/telemetry`

### Recommendations

- `GET /recommendations/targets/{target_id}`

### Planning

- `POST /tasks/propose`
- `GET /tasks/{id}`
- `GET /tasks/targets/{target_id}`
- `POST /tasks/{id}/approve`
- `POST /tasks/{id}/reject`

## Current notes

- The current schema uses plain `longitude` and `latitude` columns, not PostGIS geometry types.
- Docker files still exist as stubs, but the current recommended path is native local execution.
- `apps/frontend` is now a real live board, not a placeholder shell.
