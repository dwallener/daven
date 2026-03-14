# **Codex Work Order**

Project: **IDAP (Integrated Detection-to-Action Platform)**

Repository: daven

Goal: scaffold a working MVP implementing:

* geospatial UOP map

* detection ingestion

* target nomination

* workflow board

* asset registry

* asset recommendation

* task proposal

* approval flow

* simulated execution

* BDA assessment

# **Phase 0 — Repository Setup**

### **Task 0.1 — Create repository structure**

Create the following directory structure inside the repo root:

daven/  
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
    domain-models/  
    event-contracts/  
    geo-utils/  
  infra/  
    docker/  
    k8s/  
    db/  
  docs/  
    architecture/  
    api/  
    runbooks/

Commit this initial structure.

### **Task 0.2 — Initialize language environments**

Use:

Backend: **Rust (Axum)**

Frontend: **React \+ TypeScript**

Commands:

cargo new services/workflow-service  
cargo new services/asset-service  
cargo new services/recommendation-service  
cargo new services/planning-service  
cargo new services/ingest-service  
cargo new services/fusion-service  
cargo new services/execution-adapter  
cargo new services/assessment-service  
cargo new services/media-service  
cargo new services/auth-audit-service  
cargo new apps/api-gateway

Frontend:

cd apps  
npx create-react-app frontend \--template typescript

### **Task 0.3 — Setup shared domain models library**

Create libs/domain-models.

Define Rust structs for:

Detection  
Target  
Asset  
Recommendation  
Task  
Assessment  
Board  
User

Expose them via a crate:

pub mod detection;  
pub mod target;  
pub mod asset;  
pub mod task;  
pub mod recommendation;  
pub mod assessment;

All services must depend on this crate.

### **Task 0.4 — Setup event contracts library**

Create libs/event-contracts.

Define event types:

DetectionCreated  
AssetUpdated  
TargetNominated  
TargetTransitioned  
RecommendationGenerated  
TaskProposed  
TaskApproved  
TaskExecuted  
AssessmentCreated

Include a shared envelope:

struct EventEnvelope\<T\> {  
  event\_id: String,  
  event\_type: String,  
  occurred\_at: DateTime\<Utc\>,  
  producer: String,  
  payload: T  
}

# **Phase 1 — Infrastructure**

### **Task 1.1 — Database**

Use PostgreSQL \+ PostGIS.

Create SQL migration files in:

infra/db/migrations/

Tables required:

detections  
targets  
target\_state\_history  
assets  
tasks  
assessments  
boards  
media\_objects  
audit\_events

Enable PostGIS extension.

### **Task 1.2 — Event bus**

Use **NATS**.

Create docker compose file:

infra/docker/docker-compose.yml

Services:

* postgres  
* nats  
* redis  
* minio

Include volumes.

### **Task 1.3 — Shared config loader**

Create libs/config.

Services should read:

DATABASE\_URL  
NATS\_URL  
REDIS\_URL  
S3\_ENDPOINT  
AUTH\_SECRET

# **Phase 2 — Workflow Core**

### **Task 2.1 — Implement workflow service**

Location:

services/workflow-service

Responsibilities:

* create targets  
* manage boards  
* enforce state transitions  
* track state history

Endpoints:

POST /targets  
GET /targets/{id}  
POST /targets/{id}/transition  
GET /boards  
GET /boards/{id}

Implement allowed state transitions.

Persist transitions in target\_state\_history.

### **Task 2.2 — Implement nomination endpoint**

Add endpoint:

POST /detections/{id}/nominate

Behavior:

1. fetch detection  
2. create target  
3. set status \= NOMINATED  
4. emit TargetNominated event

### **Task 2.3 — Implement board view**

Endpoint:

GET /boards/{id}/targets

Return targets grouped by status.

# **Phase 3 — Asset Registry**

### **Task 3.1 — Implement asset service**

Location:

services/asset-service

Endpoints:

GET /assets  
GET /assets/{id}  
POST /assets  
PATCH /assets/{id}  
POST /assets/{id}/telemetry

Store:

* location  
* heading  
* speed  
* fuel  
* munitions  
* capabilities

Emit:

AssetUpdated

# **Phase 4 — Recommendation Engine**

### **Task 4.1 — Implement recommendation service**

Location:

services/recommendation-service

Endpoint:

POST /targets/{id}/recommend-assets

Steps:

1. load target  
2. load assets  
3. compute score

Score formula:

score \=  
  w\_effect\_match \* effect\_match \+  
  w\_time\_to\_target \* normalized\_time \+  
  w\_distance \* normalized\_distance \+  
  w\_endurance \* endurance \+  
  w\_munition \* munition\_available

Return ranked list.

Emit:

RecommendationGenerated

### **Task 4.2 — Explanation output**

Return explanation per candidate:

{  
  "asset\_id": "asset\_1",  
  "rank": 1,  
  "score": 0.88,  
  "features": {  
    "distance\_km": 3.8,  
    "time\_to\_target\_min": 6.2,  
    "munition\_match": true  
  }  
}

# **Phase 5 — Planning Service**

### **Task 5.1 — Implement task proposal**

Location:

services/planning-service

Endpoint:

POST /targets/{id}/propose-task

Input:

asset\_id  
task\_type  
weapon\_selection  
aimpoints

Steps:

1. fetch target  
2. fetch asset  
3. calculate route  
4. estimate time-on-target  
5. create task object

Emit:

TaskProposed

# **Phase 6 — Approval**

### **Task 6.1 — Implement approval endpoint**

In workflow service or planning service:

POST /tasks/{id}/approve  
POST /tasks/{id}/reject

Only allow users with approver role.

Emit events:

TaskApproved  
TaskRejected

# **Phase 7 — Execution Adapter**

### **Task 7.1 — Simulated execution**

Location:

services/execution-adapter

When task approved:

1. wait simulated delay  
2. emit:  
   TaskExecuted

Optionally generate fake strike evidence.

# **Phase 8 — Assessment Service**

### **Task 8.1 — Implement BDA submission**

Endpoint:

POST /tasks/{id}/assess

Input:

result  
confidence  
notes  
media\_refs

Update target state:

PENDING\_BDA → ASSESSED\_COMPLETE

Emit:

AssessmentCreated

# **Phase 9 — API Gateway**

### **Task 9.1 — Implement gateway**

Location:

apps/api-gateway

Responsibilities:

* route requests to services  
* auth enforcement  
* WebSocket push updates

Routes:

/api/targets  
/api/boards  
/api/assets  
/api/recommendations  
/api/tasks  
/api/assessments

# **Phase 10 — Frontend**

### **Task 10.1 — Map UI**

Use:

Mapbox GL  
or deck.gl

Features:

* asset markers  
* target markers  
* detection overlays  
* layer toggles

### **Task 10.2 — Workflow board**

Implement Kanban UI:

Columns:

NOMINATED  
TRIAGED  
PENDING\_PAIRING  
PAIRED  
PLAN\_DRAFTED  
PENDING\_APPROVAL  
APPROVED  
IN\_EXECUTION  
PENDING\_BDA  
COMPLETE

Cards represent targets.

### **Task 10.3 — Target detail panel**

Panel sections:

Overview  
Location  
Evidence  
Asset recommendations  
Task planning  
Approval  
Assessment

### **Task 10.4 — Asset recommendation panel**

Display ranked assets.

Allow operator weight tuning:

distance  
time\_to\_target  
effect\_match  
munition  
endurance

### **Task 10.5 — Planning workspace**

Map shows:

* target  
* asset  
* engagement line  
* estimated timeline

# **Phase 11 — Media Handling**

### **Task 11.1 — Media service**

Endpoints:

POST /media  
GET /media/{id}  
GET /media/{id}/signed-url

Use S3-compatible storage.

# **Phase 12 — Observability**

### **Task 12.1 — Logging**

Add structured logs.

Fields:

service  
request\_id  
user\_id  
target\_id  
task\_id  
action  
timestamp

### **Task 12.2 — Metrics**

Expose Prometheus metrics:

targets\_created\_total  
recommendations\_generated\_total  
task\_approvals\_total  
task\_execution\_latency  
bda\_completion\_time

# **Phase 13 — Seed Data**

Add development seed script:

scripts/seed\_data.rs

Create:

* sample assets  
* sample detections  
* sample board

# **Phase 14 — First Demo Scenario**

Implement scenario:

1. replay CV detections  
2. nominate detection  
3. card appears on board  
4. recommend assets  
5. propose task  
6. approve task  
7. simulate execution  
8. submit BDA

# **Phase 15 — Deliverables**

At end of MVP build the system should allow:

* ingest detection  
* nominate target  
* manage workflow board  
* recommend asset  
* propose task  
* approve task  
* simulate execution  
* submit assessment

# **Final Codex Instruction**

Execute phases **in order**.

Do **not** build planning, recommendation, or BDA until the **workflow spine works**.

The system must first prove it can:

Detection → Target → Board → State transitions

Everything else builds on that spine.

