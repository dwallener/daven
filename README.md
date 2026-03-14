# Daven

Daven is an MVP scaffold for an integrated detection-to-action platform.

Current scope:

- Rust workspace for backend services and shared libraries
- Workflow service with in-memory target lifecycle management
- Infrastructure stubs for Postgres/PostGIS, NATS, Redis, and MinIO
- Frontend shell for the future React UI

## Workspace layout

- `apps/api-gateway`: placeholder API gateway crate
- `apps/frontend`: React + TypeScript shell
- `libs/config`: shared runtime configuration
- `libs/domain-models`: canonical domain types
- `libs/event-contracts`: shared event payloads and envelope
- `services/workflow-service`: workflow spine service
- `infra/docker`: local dependency stack
- `infra/db/migrations`: initial schema migrations

## Workflow service

The first implemented slice covers:

- create target
- fetch target
- transition target state
- list boards
- list targets grouped by board status
- nominate a target from a detection reference

The service currently uses in-memory storage so the API and lifecycle rules can be exercised before persistence is wired in.
