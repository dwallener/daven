# **Implementation Blueprint**

**Project Codename:** IDAP

**Scope:**   
Detection → nomination → workflow → asset recommendation → task planning → approval → execution tracking → BDA

# **1\. Implementation Goals**

This blueprint defines:

* service boundaries  
* canonical data model  
* event contracts  
* storage choices  
* API surface  
* UI/backend interaction model  
* deployment topology  
* MVP build sequence

This version assumes:

* multiple input feeds, initially stubbed or replayed  
* human approval required before action  
* execution may be simulated in MVP  
* system is geospatial-first and event-driven

# **2\. Deployment Model**

Use a service-oriented architecture, but keep the first cut sane. Do not spawn 40 microservices because architecture astronauts get lonely.

## **2.1 MVP topology**

For MVP, use:

* 1 frontend app  
* 1 API gateway/backend-for-frontend  
* 1 ingest service  
* 1 fusion service  
* 1 workflow service  
* 1 recommendation service  
* 1 planning service  
* 1 media service  
* 1 execution adapter service  
* 1 assessment service  
* 1 auth/audit service  
* shared event bus  
* shared PostgreSQL/PostGIS  
* shared object store  
* Redis for cache/session/pubsub if needed

## **2.2 Suggested runtime**

* Backend: Rust or Go for core services  
* Frontend: React \+ TypeScript  
* Realtime transport: WebSocket or SSE  
* Primary DB: PostgreSQL \+ PostGIS  
* Event bus: NATS / Redpanda / Kafka  
* Cache: Redis  
* Object storage: S3-compatible  
* Search/logging: OpenSearch or ELK  
* Metrics: Prometheus \+ Grafana

For an MVP, **NATS** is a nice sweet spot. Kafka is great if you enjoy operating a distributed tax form.

# **3\. Service Decomposition**

## **3.1 API Gateway / BFF**

Purpose:

* frontend entrypoint  
* auth enforcement  
* response aggregation  
* realtime subscription bootstrap

Responsibilities:

* issue JWT/session tokens  
* aggregate data across services  
* expose frontend-oriented endpoints  
* manage websocket channels

Does not own domain truth.

## **3.2 Ingest Service**

Purpose:

* ingest external feeds  
* validate and normalize external payloads  
* publish canonical events

Inputs:

* FMV metadata  
* CV detections  
* radar tracks  
* SIGINT events  
* asset telemetry  
* environmental layers  
* analyst annotations

Outputs:

* canonical event messages on bus  
* raw payload references in object store if needed

## **3.3 Fusion Service**

Purpose:

* correlate detections/tracks across sources  
* create fused candidates  
* update target candidate confidence

Responsibilities:

* dedup spatial/temporal duplicates  
* merge related detections  
* emit candidate promotion suggestions  
* maintain confidence/provenance graph

Outputs:

* fusion.candidate.created  
* fusion.candidate.updated

## **3.4 Workflow Service**

Purpose:

* own target lifecycle state machine  
* own boards and card placement  
* own assignment, status, history

Responsibilities:

* create targets from nominations  
* validate transitions  
* persist state history  
* maintain board/card views  
* attach comments, tags, operator actions

This is the core “truth” service for targets.

## **3.5 Asset Registry Service**

Purpose:

* maintain live and historical asset state

Responsibilities:

* asset metadata  
* telemetry updates  
* readiness state  
* capability inventory  
* munition/fuel status  
* domain constraints

Outputs:

* asset query endpoints  
* asset state events

## **3.6 Recommendation Service**

Purpose:

* rank assets and optionally COAs for a target

Responsibilities:

* retrieve target \+ asset state  
* apply weighted scoring  
* explain results  
* store recommendation snapshots

Outputs:

* ranked candidate list  
* explanation objects  
* recommendation events

## **3.7 Planning Service**

Purpose:

* convert selected asset-target pairings into actionable plans/tasks

Responsibilities:

* route geometry  
* timing estimation  
* weapon/effect selection  
* multi-asset coordination  
* task draft generation

Outputs:

* plan draft  
* timeline block  
* proposed task

## **3.8 Execution Adapter Service**

Purpose:

* translate approved tasks into downstream system payloads

Responsibilities:

* map internal task object to external tasking schema  
* send to simulator or external command/control system  
* receive execution acknowledgements/status

For MVP:

* execution can be simulated internally

## **3.9 Assessment Service**

Purpose:

* manage BDA and post-action assessment

Responsibilities:

* before/after evidence sets  
* manual and AI-assisted assessment  
* status closure  
* confidence tracking  
* reopen flow if inconclusive

## **3.10 Media Service**

Purpose:

* manage imagery/video references and derived artifacts

Responsibilities:

* clip registration  
* thumbnail generation  
* frame linking  
* evidence bundle assembly  
* signed URL generation

## **3.11 Audit/Auth Service**

Purpose:

* user identity, roles, approvals, immutable audit

Responsibilities:

* RBAC  
* approval authority checks  
* signed approvals  
* user action logging  
* classification labels  
* access decisions

# **4\. Canonical Domain Model**

Below is the implementation-oriented domain model.

## **4.1 Detection**

{  
  "id": "det\_123",  
  "source\_type": "cv\_fmv",  
  "source\_id": "feed\_alpha",  
  "external\_ref": "frame\_455:obj\_12",  
  "timestamp": "2026-03-13T15:01:22Z",  
  "geometry": {  
    "type": "Point",  
    "coordinates": \[-122.91, 49.21\]  
  },  
  "bbox": \[0.41, 0.33, 0.49, 0.57\],  
  "classification": "vehicle",  
  "subclassification": "unknown",  
  "confidence": 0.87,  
  "sensor\_metadata": {  
    "platform\_id": "uav\_7",  
    "sensor\_mode": "EO",  
    "heading\_deg": 141.0  
  },  
  "media\_refs": \["media\_frame\_455"\],  
  "provenance": {  
    "model": "cv-detector-v3",  
    "raw\_event\_id": "raw\_abc"  
  }  
}

## **4.2 Fused Candidate**

{  
  "id": "cand\_901",  
  "detection\_ids": \["det\_123", "det\_124"\],  
  "geometry": {  
    "type": "Point",  
    "coordinates": \[-122.91, 49.21\]  
  },  
  "classification": "vehicle\_cluster",  
  "confidence": 0.91,  
  "first\_seen": "2026-03-13T15:01:22Z",  
  "last\_seen": "2026-03-13T15:03:01Z",  
  "supporting\_sources": \["cv\_fmv", "radar\_track"\]  
}

## **4.3 Target**

{  
  "id": "tgt\_5001",  
  "candidate\_id": "cand\_901",  
  "status": "PENDING\_PAIRING",  
  "board\_id": "dynamic\_main",  
  "title": "Computer Vision Detection \- Vehicle",  
  "classification": "vehicle",  
  "priority": 72,  
  "location": {  
    "type": "Point",  
    "coordinates": \[-122.91, 49.21\]  
  },  
  "aimpoints": \[  
    {  
      "id": "ap\_1",  
      "geometry": { "type": "Point", "coordinates": \[-122.9101, 49.2102\] }  
    }  
  \],  
  "supporting\_detection\_ids": \["det\_123", "det\_124"\],  
  "intelligence\_refs": \[\],  
  "created\_by": "user\_12",  
  "created\_at": "2026-03-13T15:05:10Z",  
  "labels": \["dynamic", "vehicle"\],  
  "state\_history": \[  
    {  
      "from": "NOMINATED",  
      "to": "PENDING\_PAIRING",  
      "at": "2026-03-13T15:05:12Z",  
      "by": "user\_12"  
    }  
  \]  
}

## **4.4 Asset**

{  
  "id": "asset\_stryker\_1",  
  "callsign": "Stryker1",  
  "platform\_type": "stryker",  
  "domain": "land",  
  "location": {  
    "type": "Point",  
    "coordinates": \[-122.89, 49.19\]  
  },  
  "heading\_deg": 22,  
  "speed\_kph": 45,  
  "availability": "AVAILABLE",  
  "capabilities": \["observe", "strike"\],  
  "munitions": \[  
    { "type": "50cal", "count": 600 }  
  \],  
  "fuel\_pct": 78,  
  "time\_on\_station\_min": 240,  
  "constraints": \["road\_access\_required"\],  
  "updated\_at": "2026-03-13T15:06:00Z"  
}

## **4.5 Recommendation**

{  
  "id": "rec\_44",  
  "target\_id": "tgt\_5001",  
  "generated\_at": "2026-03-13T15:06:12Z",  
  "weights": {  
    "effect\_match": 0.35,  
    "time\_to\_target": 0.25,  
    "distance": 0.15,  
    "endurance": 0.10,  
    "munition\_available": 0.15  
  },  
  "candidates": \[  
    {  
      "asset\_id": "asset\_stryker\_1",  
      "score": 0.88,  
      "rank": 1,  
      "explanation": {  
        "effect\_match": 0.95,  
        "time\_to\_target\_min": 6.2,  
        "distance\_km": 3.8,  
        "munition\_ok": true  
      }  
    }  
  \]  
}

## **4.6 Task**

{  
  "id": "task\_888",  
  "target\_id": "tgt\_5001",  
  "asset\_ids": \["asset\_stryker\_1"\],  
  "task\_type": "SMACK",  
  "effect\_type": "kinetic",  
  "weapon\_selection": "50cal",  
  "status": "PENDING\_APPROVAL",  
  "approval\_status": "REQUIRED",  
  "time\_on\_target": "2026-03-13T15:14:00Z",  
  "timeline\_geometry": {  
    "type": "LineString",  
    "coordinates": \[  
      \[-122.89, 49.19\],  
      \[-122.91, 49.21\]  
    \]  
  },  
  "aimpoint\_ids": \["ap\_1"\],  
  "created\_by": "user\_15",  
  "created\_at": "2026-03-13T15:08:33Z"  
}

## **4.7 Assessment**

{  
  "id": "bda\_77",  
  "task\_id": "task\_888",  
  "target\_id": "tgt\_5001",  
  "result": "DESTROYED",  
  "confidence": 0.82,  
  "assessment\_type": "manual\_plus\_sensor",  
  "media\_refs": \["clip\_after\_99"\],  
  "notes": "Visible blast and target obscuration; no continued movement observed.",  
  "created\_by": "user\_20",  
  "created\_at": "2026-03-13T15:17:20Z"  
}

# **5\. Database Schema**

Use PostgreSQL \+ PostGIS. Below is the practical table set.

## **5.1 Core tables**

* users

* roles

* user\_roles

* boards

* targets

* target\_state\_history

* detections

* fused\_candidates

* candidate\_detection\_links

* assets

* asset\_telemetry

* recommendations

* recommendation\_candidates

* tasks

* task\_approvals

* task\_execution\_updates

* assessments

* media\_objects

* annotations

* audit\_events

## **5.2 Example DDL sketch**

### **targets**

create table targets (  
  id text primary key,  
  candidate\_id text,  
  board\_id text not null references boards(id),  
  title text not null,  
  status text not null,  
  classification text,  
  priority int not null default 0,  
  location geometry(Point, 4326),  
  created\_by text not null,  
  created\_at timestamptz not null,  
  updated\_at timestamptz not null default now(),  
  labels jsonb not null default '\[\]'::jsonb,  
  metadata jsonb not null default '{}'::jsonb  
);

create index idx\_targets\_status on targets(status);  
create index idx\_targets\_board on targets(board\_id);  
create index idx\_targets\_location on targets using gist(location);

### **detections**

create table detections (  
  id text primary key,  
  source\_type text not null,  
  source\_id text not null,  
  external\_ref text,  
  ts timestamptz not null,  
  geometry geometry(Geometry, 4326\) not null,  
  bbox jsonb,  
  classification text,  
  subclassification text,  
  confidence double precision,  
  sensor\_metadata jsonb not null default '{}'::jsonb,  
  provenance jsonb not null default '{}'::jsonb,  
  created\_at timestamptz not null default now()  
);

create index idx\_detections\_ts on detections(ts desc);  
create index idx\_detections\_geom on detections using gist(geometry);

### **assets**

create table assets (  
  id text primary key,  
  callsign text,  
  platform\_type text not null,  
  domain text not null,  
  location geometry(Point, 4326),  
  heading\_deg double precision,  
  speed\_kph double precision,  
  availability text not null,  
  capabilities jsonb not null default '\[\]'::jsonb,  
  munitions jsonb not null default '\[\]'::jsonb,  
  fuel\_pct double precision,  
  time\_on\_station\_min int,  
  constraints jsonb not null default '\[\]'::jsonb,  
  updated\_at timestamptz not null default now()  
);

create index idx\_assets\_location on assets using gist(location);  
create index idx\_assets\_availability on assets(availability);

### **tasks**

create table tasks (  
  id text primary key,  
  target\_id text not null references targets(id),  
  task\_type text not null,  
  effect\_type text,  
  weapon\_selection text,  
  status text not null,  
  approval\_status text not null,  
  time\_on\_target timestamptz,  
  timeline\_geometry geometry(LineString, 4326),  
  created\_by text not null,  
  created\_at timestamptz not null,  
  updated\_at timestamptz not null default now(),  
  metadata jsonb not null default '{}'::jsonb  
);

create index idx\_tasks\_target on tasks(target\_id);  
create index idx\_tasks\_status on tasks(status);

# **6\. Event Bus Design**

Use event-driven contracts for cross-service updates.

## **6.1 Topics / subjects**

For NATS:

* ingest.detection.created

* ingest.asset.updated

* fusion.candidate.created

* fusion.candidate.updated

* workflow.target.nominated

* workflow.target.transitioned

* recommendation.generated

* planning.task.proposed

* planning.task.updated

* approval.task.approved

* approval.task.rejected

* execution.task.status

* assessment.created

* audit.user.action

## **6.2 Event envelope**

All events share a common envelope:

{  
  "event\_id": "evt\_001",  
  "event\_type": "workflow.target.transitioned",  
  "occurred\_at": "2026-03-13T15:05:12Z",  
  "producer": "workflow-service",  
  "schema\_version": "1.0",  
  "correlation\_id": "corr\_abc",  
  "payload": {}  
}

## **6.3 Example event**

{  
  "event\_id": "evt\_101",  
  "event\_type": "workflow.target.transitioned",  
  "occurred\_at": "2026-03-13T15:05:12Z",  
  "producer": "workflow-service",  
  "schema\_version": "1.0",  
  "correlation\_id": "corr\_target\_5001",  
  "payload": {  
    "target\_id": "tgt\_5001",  
    "from\_status": "NOMINATED",  
    "to\_status": "PENDING\_PAIRING",  
    "board\_id": "dynamic\_main",  
    "acted\_by": "user\_12"  
  }  
}

# **7\. Service APIs**

Use REST for CRUD/workflow and WebSocket/SSE for realtime. You can add gRPC internally later if you enjoy protobufs and misery equally.

## **7.1 API Gateway endpoints**

### **Auth**

* POST /api/auth/login

* POST /api/auth/refresh

* POST /api/auth/logout

* GET /api/me

### **Map/UOP**

* GET /api/uop/layers

* GET /api/uop/objects?bbox=...

* GET /api/uop/assets?bbox=...

* GET /api/uop/detections?bbox=...

* GET /api/uop/targets?bbox=...

### **Detections / Nomination**

* GET /api/detections/{id}

* POST /api/detections/{id}/nominate

* POST /api/candidates/{id}/nominate

### **Targets / Workflow**

* GET /api/boards

* GET /api/boards/{id}

* GET /api/targets/{id}

* POST /api/targets/{id}/transition

* POST /api/targets/{id}/comment

* POST /api/targets/{id}/annotation

### **Recommendations**

* POST /api/targets/{id}/recommend-assets

* GET /api/recommendations/{id}

### **Planning**

* POST /api/targets/{id}/propose-task

* GET /api/tasks/{id}

* POST /api/tasks/{id}/update

### **Approval**

* POST /api/tasks/{id}/approve

* POST /api/tasks/{id}/reject

* POST /api/tasks/{id}/retask

### **Execution**

* POST /api/tasks/{id}/execute

* GET /api/tasks/{id}/execution-status

### **Assessment**

* POST /api/tasks/{id}/assess

* GET /api/targets/{id}/assessments

### **Media**

* GET /api/media/{id}

* POST /api/media/clips

* GET /api/media/{id}/signed-url

# **8\. Workflow Rules**

## **8.1 Allowed target transitions**

NEW\_DETECTION \-\> NOMINATED  
NOMINATED \-\> TRIAGED  
TRIAGED \-\> PENDING\_PAIRING  
PENDING\_PAIRING \-\> PAIRED  
PAIRED \-\> PLAN\_DRAFTED  
PLAN\_DRAFTED \-\> PENDING\_APPROVAL  
PENDING\_APPROVAL \-\> APPROVED  
PENDING\_APPROVAL \-\> REJECTED  
APPROVED \-\> IN\_EXECUTION  
IN\_EXECUTION \-\> PENDING\_BDA  
PENDING\_BDA \-\> ASSESSED\_COMPLETE  
PENDING\_BDA \-\> TRIAGED

## **8.2 Transition policy examples**

* only operators/analysts may nominate  
* only planners may move to PLAN\_DRAFTED  
* only approval authority may move to APPROVED  
* only assessment role may close to ASSESSED\_COMPLETE

Implement this in Workflow Service, not in frontend fairy dust.

# **9\. Recommendation Engine Design**

## **9.1 Inputs**

* target geometry  
* target class  
* target priority  
* asset location  
* asset availability  
* asset capabilities  
* asset munitions  
* time-to-target estimate  
* weather/terrain constraints  
* operator weights

## **9.2 First-pass scoring function**

score \=  
  w\_effect\_match \* effect\_match \+  
  w\_time \* normalized\_time\_to\_target \+  
  w\_distance \* normalized\_distance \+  
  w\_endurance \* endurance\_score \+  
  w\_munition \* munition\_score \+  
  w\_risk \* inverse\_risk\_score

Normalize each feature to 0..1.

## **9.3 Explainability output**

Every candidate returns:

* raw feature values  
* normalized feature values  
* weighted contribution  
* hard constraints that disqualified it

Example:

{  
  "asset\_id": "asset\_stryker\_1",  
  "rank": 1,  
  "score": 0.88,  
  "features": {  
    "effect\_match": { "raw": 1.0, "normalized": 1.0, "weighted": 0.35 },  
    "time\_to\_target\_min": { "raw": 6.2, "normalized": 0.81, "weighted": 0.20 },  
    "distance\_km": { "raw": 3.8, "normalized": 0.77, "weighted": 0.11 }  
  },  
  "constraints": \[\]  
}

## **9.4 Operator tuning**

Allow per-request weights. Store them with the recommendation snapshot so later nobody has to ask, “Why did it choose the clown car with a machine gun?”

# **10\. Planning Engine Design**

## **10.1 Inputs**

* selected target  
* selected asset(s)  
* chosen task type  
* operator parameters  
* routing constraints  
* aimpoints  
* schedule constraints

## **10.2 Outputs**

* draft task object  
* engagement line geometry  
* estimated time on target  
* selected weapon/effect  
* optional deconfliction warnings

## **10.3 MVP heuristics**

For MVP, support:

* single target  
* single asset  
* simple straight-line or network-assisted route estimate  
* single task type  
* simple time-on-target

Later:

* multi-asset synchronized plans  
* route optimization  
* conflict resolution  
* reserve asset assignment

# **11\. UI Architecture**

## **11.1 Frontend stack**

* React  
* TypeScript  
* Mapbox GL JS or deck.gl \+ map engine  
* Zustand or Redux Toolkit  
* React Query / TanStack Query  
* WebSocket/SSE client for realtime

## **11.2 Frontend module breakdown**

* uop-map  
* layer-controls  
* sensor-inspector  
* detection-overlay  
* target-board  
* target-detail-panel  
* asset-recommender  
* planning-workspace  
* approval-panel  
* assessment-workspace  
* activity-timeline

## **11.3 Realtime subscriptions**

Subscribe to:

* target updates  
* task updates  
* asset updates  
* recommendation generation  
* assessment updates

# **12\. UI State Model**

Keep UI state separate from domain truth.

## **12.1 Server truth**

* targets  
* boards  
* tasks  
* assets  
* detections  
* recommendations

## **12.2 Client-local state**

* selected target  
* open panels  
* map filters  
* measurement tools  
* draft weights before submit  
* viewport  
* unsaved annotations

# **13\. Realtime Update Strategy**

## **13.1 WebSocket/SSE message types**

* target.updated  
* board.updated  
* asset.updated  
* task.updated  
* recommendation.ready  
* assessment.updated

## **13.2 Example UI push message**

{  
  "type": "target.updated",  
  "target\_id": "tgt\_5001",  
  "changes": {  
    "status": "PAIRED"  
  }  
}

UI then refetches or patches local cache.

# **14\. Ingest Adapter Contracts**

Each source adapter transforms source-native payload into canonical event(s).

## **14.1 FMV/CV adapter**

Input:

* frame timestamp  
* platform metadata  
* bounding box detections

Output:

* detection events  
* media registration events

## **14.2 Radar adapter**

Input:

* track ID  
* lat/lon  
* speed  
* heading  
* uncertainty

Output:

* track/detection event

## **14.3 SIGINT adapter**

Input:

* emitter event  
* confidence  
* geolocation estimate

Output:

* signal-derived detection or intel event

## **14.4 Asset telemetry adapter**

Input:

* platform ID  
* position  
* readiness  
* fuel  
* munitions

Output:

* asset update event

# **15\. Media Handling**

## **15.1 Object store layout**

/raw/fmv/{feed\_id}/{date}/...  
/derived/thumbnails/{media\_id}.jpg  
/derived/clips/{clip\_id}.mp4  
/derived/frames/{frame\_id}.jpg  
/evidence/{target\_id}/{bundle\_id}/...

## **15.2 Metadata table**

media\_objects

* id  
* type  
* storage\_key  
* source\_feed  
* timestamp  
* width  
* height  
* duration\_ms  
* associated\_entity\_type  
* associated\_entity\_id

## **15.3 Evidence bundles**

Allow packaging of:

* before image  
* strike clip  
* after image  
* assessment note  
* provenance manifest

# **16\. Auth and Security**

## **16.1 RBAC roles**

* operator  
* analyst  
* planner  
* approver  
* assessor  
* admin

## **16.2 Permission matrix examples**

* operator: inspect, measure, nominate  
* analyst: annotate, triage, enrich  
* planner: request recommendations, draft tasks  
* approver: approve/reject/retask  
* assessor: submit BDA, close tasks

## **16.3 Audit requirements**

Audit every:

* login  
* nomination  
* transition  
* recommendation request  
* task proposal  
* approval/rejection  
* execution status update  
* assessment submission

Use append-only audit store or at least treat it like one.

# **17\. Observability**

## **17.1 Metrics per service**

### **Ingest**

* events/sec  
* dropped events  
* parse failures  
* latency to publish

### **Fusion**

* candidate merge count  
* average candidate age  
* false merge overrides

### **Workflow**

* targets by status  
* transition latency  
* cards per board  
* stale targets

### **Recommendation**

* generation latency  
* recommendation requests/sec  
* candidate count per recommendation

### **Planning**

* task draft latency  
* route generation errors

### **Execution**

* task ack latency  
* execution status lag

### **Assessment**

* BDA turnaround time  
* reopen rate

## **17.2 Distributed tracing**

Use OpenTelemetry. Otherwise debugging event chains becomes archaeological work.

# **18\. API Examples**

## **18.1 Nominate detection**

POST /api/detections/det\_123/nominate

Request:

{  
  "board\_id": "dynamic\_main",  
  "title": "Computer Vision Detection \- Vehicle",  
  "labels": \["dynamic", "vehicle"\]  
}

Response:

{  
  "target\_id": "tgt\_5001",  
  "status": "NOMINATED"  
}

## **18.2 Transition target**

POST /api/targets/tgt\_5001/transition

Request:

{  
  "to\_status": "PENDING\_PAIRING",  
  "reason": "Validated and ready for asset matching"  
}

## **18.3 Recommend assets**

POST /api/targets/tgt\_5001/recommend-assets

Request:

{  
  "weights": {  
    "effect\_match": 0.35,  
    "time\_to\_target": 0.25,  
    "distance": 0.15,  
    "endurance": 0.10,  
    "munition\_available": 0.15  
  }  
}

## **18.4 Propose task**

POST /api/targets/tgt\_5001/propose-task

Request:

{  
  "asset\_id": "asset\_stryker\_1",  
  "task\_type": "SMACK",  
  "weapon\_selection": "50cal",  
  "aimpoint\_ids": \["ap\_1"\]  
}

## **18.5 Approve task**

POST /api/tasks/task\_888/approve

Request:

{  
  "note": "Approved for execution"  
}

# **19\. Execution Integration Pattern**

For MVP, support two modes.

## **19.1 Simulated execution mode**

* approved task enters simulator  
* after delay, emit execution.task.status  
* media feed or mock strike evidence generated  
* assessment may proceed

## **19.2 External system mode**

* transform task into external schema  
* send via adapter  
* receive ack/update callbacks  
* store external refs in task metadata

Task metadata example:

{  
  "external\_system": "firesim\_v1",  
  "external\_task\_ref": "fs-2918"  
}

# **20\. Assessment Workflow**

## **20.1 Inputs**

* pre-action evidence  
* post-action evidence  
* analyst note  
* optional AI damage classifier

## **20.2 Results**

* DESTROYED  
* DAMAGED  
* NO\_EFFECT  
* INCONCLUSIVE  
* MISSED

## **20.3 Closure rules**

* if DESTROYED or DAMAGED, move to complete  
* if INCONCLUSIVE, remain PENDING\_BDA  
* if NO\_EFFECT or MISSED, reopen to triage or pairing

# **21\. Suggested Repository Structure**

idap/  
  apps/  
    frontend/  
    api-gateway/  
  services/  
    ingest-service/  
    fusion-service/  
    workflow-service/  
    asset-service/  
    recommendation-service/  
    planning-service/  
    execution-adapter/  
    assessment-service/  
    media-service/  
    auth-audit-service/  
  libs/  
    event-contracts/  
    domain-models/  
    geo-utils/  
    auth-common/  
    ui-components/  
  infra/  
    docker/  
    k8s/  
    terraform/  
    monitoring/  
  docs/  
    architecture/  
    api/  
    runbooks/

# **22\. MVP Build Order**

Do not build this sideways. Use this order.

## **Phase 1: Domain and storage**

* canonical schemas  
* Postgres/PostGIS setup  
* audit model  
* event envelope

## **Phase 2: Workflow spine**

* workflow service  
* board CRUD  
* target nomination  
* target transitions  
* history tracking

## **Phase 3: UOP baseline**

* map UI  
* layer toggles  
* target/detection rendering  
* selection model

## **Phase 4: Detections and ingest**

* FMV/CV replay adapter  
* detection persistence  
* detection overlay  
* nominate from detection

## **Phase 5: Assets**

* asset registry  
* asset telemetry updates  
* map rendering for assets

## **Phase 6: Recommendation**

* weighted heuristic engine  
* ranked asset list  
* explanation panel

## **Phase 7: Planning \+ approval**

* task drafts  
* timeline  
* approval/reject flow

## **Phase 8: Execution simulation**

* simulated execution adapter  
* status updates  
* task lifecycle integration

## **Phase 9: Assessment**

* before/after evidence  
* BDA note UI  
* close/reopen flow

## **Phase 10: Hardening**

* RBAC  
* audit completeness  
* observability  
* performance tuning

# **23\. MVP Non-Goals**

Avoid these early:

* full autonomous planning  
* real weapons integration  
* multi-organization federation  
* advanced ML COA generation  
* swarm orchestration  
* predictive campaign modeling  
* fully automated BDA closure

That stuff comes later, after the basics stop catching fire.

# **24\. Concrete First Sprint Deliverables**

A realistic Sprint 1:

* Postgres schema for detections, targets, boards, assets, tasks  
* Workflow service with nomination \+ transitions  
* API gateway with auth stub  
* React board UI  
* Map UI with target markers  
* Event bus envelope \+ 3 event types  
* Seed data loader  
* Audit logging for nomination \+ transition

Sprint 2:

* FMV/CV replay ingest  
* detection overlay  
* nominate from detection  
* asset registry \+ rendering  
* recommendation endpoint

Sprint 3:

* propose task  
* approval flow  
* execution simulator  
* assessment entry

# **25\. Minimal Tech Stack Recommendation**

If you want the least drama:

* Frontend: React \+ TypeScript \+ Mapbox/deck.gl  
* Backend services: Rust with Axum or Go with Fiber/Chi  
* DB: Postgres/PostGIS  
* Bus: NATS  
* Cache: Redis  
* Storage: MinIO/S3  
* Auth: Keycloak or internal JWT for MVP  
* Metrics: Prometheus/Grafana

If you want fastest prototyping and are willing to pay some future cleanup tax:

* Backend: Python FastAPI for gateway \+ workflow/recommendation  
* same DB/bus/storage  
* later migrate hot-path services to Rust/Go

# **26\. Final Engineering Principle**

The system should be built around **stateful geospatial objects and evented transitions**, not around screens.

Screens are temporary.

Objects and transitions are the real machine.

The core implementation contract is:

* ingest evidence  
* create structured objects  
* move them through governed states  
* recommend actions  
* require approval  
* record outcomes  
* preserve provenance

That’s the spine. Everything else is ornamental tactical furniture.

