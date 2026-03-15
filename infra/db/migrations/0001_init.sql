create table if not exists boards (
  id text primary key,
  name text not null,
  statuses jsonb not null default '[]'::jsonb,
  created_at timestamptz not null default now()
);

create table if not exists detections (
  id text primary key,
  source_type text not null,
  source_id text not null,
  external_ref text,
  ts timestamptz not null,
  longitude double precision not null,
  latitude double precision not null,
  classification text,
  confidence real,
  created_at timestamptz not null default now()
);

create table if not exists targets (
  id text primary key,
  board_id text not null references boards(id),
  title text not null,
  status text not null,
  classification text,
  priority int not null default 0,
  longitude double precision not null,
  latitude double precision not null,
  source_detection_id text,
  created_by text not null,
  created_at timestamptz not null,
  updated_at timestamptz not null,
  labels jsonb not null default '[]'::jsonb
);

alter table targets add column if not exists labels jsonb not null default '[]'::jsonb;

create index if not exists idx_targets_status on targets(status);
create index if not exists idx_targets_board on targets(board_id);

create table if not exists target_state_history (
  id bigserial primary key,
  target_id text not null references targets(id),
  from_status text,
  to_status text not null,
  actor text not null,
  transitioned_at timestamptz not null
);

create table if not exists assets (
  id text primary key,
  callsign text not null,
  platform_type text not null,
  domain text not null,
  longitude double precision not null,
  latitude double precision not null,
  availability text not null,
  capabilities jsonb not null default '[]'::jsonb,
  updated_at timestamptz not null
);

create table if not exists asset_telemetry (
  id bigserial primary key,
  asset_id text not null references assets(id),
  longitude double precision not null,
  latitude double precision not null,
  recorded_at timestamptz not null
);

create table if not exists recommendations (
  id text primary key,
  target_id text not null references targets(id),
  generated_at timestamptz not null,
  weights jsonb not null default '{}'::jsonb
);

create table if not exists recommendation_candidates (
  id bigserial primary key,
  recommendation_id text not null references recommendations(id) on delete cascade,
  asset_id text not null references assets(id),
  score real not null,
  rank int not null,
  explanation jsonb not null default '{}'::jsonb
);

create table if not exists task_execution_updates (
  id bigserial primary key,
  task_id text not null references tasks(id),
  execution_status text not null,
  actor text not null,
  notes text,
  created_at timestamptz not null default now()
);

create table if not exists tasks (
  id text primary key,
  target_id text not null references targets(id),
  asset_ids jsonb not null default '[]'::jsonb,
  task_type text not null,
  effect_type text not null,
  status text not null,
  approval_status text not null,
  time_on_target timestamptz,
  created_by text not null default 'system',
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

alter table tasks add column if not exists created_by text not null default 'system';
alter table tasks add column if not exists created_at timestamptz not null default now();
alter table tasks add column if not exists updated_at timestamptz not null default now();

create table if not exists assessments (
  id text primary key,
  task_id text not null references tasks(id),
  target_id text not null references targets(id),
  result text not null,
  confidence real not null,
  notes text,
  created_at timestamptz not null
);

create table if not exists media_objects (
  id text primary key,
  media_type text not null,
  object_key text not null,
  created_at timestamptz not null default now()
);

create table if not exists audit_events (
  id bigserial primary key,
  actor text not null,
  action text not null,
  entity_type text not null,
  entity_id text not null,
  payload jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);
