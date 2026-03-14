Below is a **structured backlog (\~80 tickets)** suitable for Codex or an engineering team.

Each ticket includes:

* **ID**  
* **Title**  
* **Description**  
* **Dependencies**  
* **Acceptance Criteria**

Tickets are grouped by build phase so they can be scheduled logically.

# 

# **Epic 1 — Repository & Project Setup**

### **T001 — Initialize repository structure**

Create the base folder structure described in the architecture blueprint.

Dependencies: none

Acceptance Criteria:

* directories exist  
* commit created  
* README updated

---

### **T002 — Initialize Rust workspace**

Create Rust workspace for all backend services.

Dependencies: T001

Acceptance Criteria:

* Cargo.toml workspace defined

* services compile

---

### **T003 — Initialize frontend app**

Create React \+ TypeScript app in apps/frontend.

Dependencies: T001

Acceptance Criteria:

* app runs via npm start

* TypeScript configured

---

### **T004 — Setup shared libraries folder**

Create libs/domain-models, libs/event-contracts, libs/geo-utils.

Dependencies: T002

Acceptance Criteria:

* crates compile

* services import successfully

---

### **T005 — Add common config loader**

Implement shared configuration library.

Dependencies: T004

Acceptance Criteria:

* environment variables parsed

* services start with config

---

# 

# **Epic 2 — Infrastructure**

### **T006 — Create docker-compose environment**

Define services: Postgres, NATS, Redis, MinIO.

Dependencies: T001

Acceptance Criteria:

* docker compose up launches services

* health checks pass

---

### **T007 — Setup Postgres with PostGIS**

Enable geospatial extension.

Dependencies: T006

Acceptance Criteria:

* PostGIS installed

* spatial queries succeed

---

### **T008 — Create migration framework**

Add SQL migration tool.

Dependencies: T007

Acceptance Criteria:

* migrations run automatically

* schema version tracked

---

### **T009 — Create NATS messaging client**

Add shared event bus client library.

Dependencies: T004

Acceptance Criteria:

* service publishes/subscribes to events

---

### **T010 — Setup object storage**

Configure MinIO bucket structure.

Dependencies: T006

Acceptance Criteria:

* files upload/download

* signed URLs generated

---

# 

# **Epic 3 — Domain Models**

### **T011 — Implement Detection struct**

Create Rust struct for detection.

Dependencies: T004

Acceptance Criteria:

* struct defined

* serialization working

---

### **T012 — Implement Target struct**

Create target model.

Dependencies: T011

Acceptance Criteria:

* includes lifecycle fields

* serialization working

---

### **T013 — Implement Asset struct**

Define asset model.

Dependencies: T004

Acceptance Criteria:

* includes capability metadata

---

### **T014 — Implement Task struct**

Define task model.

Dependencies: T012

Acceptance Criteria:

* links to target and assets

---

### **T015 — Implement Assessment struct**

Define BDA model.

Dependencies: T014

Acceptance Criteria:

* supports result classification

---

# 

# **Epic 4 — Database Schema**

### **T016 — Create detections table**

Define detection schema.

Dependencies: T008

Acceptance Criteria:

* spatial index present

* inserts succeed

---

### **T017 — Create targets table**

Define target schema.

Dependencies: T016

Acceptance Criteria:

* status field exists

* geospatial column

---

### **T018 — Create target\_state\_history table**

Track lifecycle transitions.

Dependencies: T017

Acceptance Criteria:

* transitions logged

---

### **T019 — Create assets table**

Persist asset metadata.

Dependencies: T017

Acceptance Criteria:

* asset location stored

---

### **T020 — Create tasks table**

Persist task plans.

Dependencies: T017

Acceptance Criteria:

* target relationship enforced

---

### **T021 — Create assessments table**

Persist BDA.

Dependencies: T020

Acceptance Criteria:

* references task and target

---

### **T022 — Create boards table**

Persist workflow boards.

Dependencies: T017

Acceptance Criteria:

* board lookup works

---

### **T023 — Create audit\_events table**

Audit trail storage.

Dependencies: T017

Acceptance Criteria:

* user actions recorded

---

# 

# **Epic 5 — Workflow Service**

### **T024 — Create workflow service scaffold**

Initialize service project.

Dependencies: T002

Acceptance Criteria:

* server starts

---

### **T025 — Implement target creation endpoint**

POST /targets

Dependencies: T024

Acceptance Criteria:

* target stored

---

### **T026 — Implement nomination endpoint**

POST /detections/{id}/nominate

Dependencies: T016, T025

Acceptance Criteria:

* detection converted to target

---

### **T027 — Implement target retrieval endpoint**

GET /targets/{id}

Dependencies: T025

Acceptance Criteria:

* target returned

---

### **T028 — Implement board retrieval endpoint**

GET /boards/{id}

Dependencies: T022

Acceptance Criteria:

* grouped targets returned

---

### **T029 — Implement lifecycle transition logic**

Implement state machine.

Dependencies: T018

Acceptance Criteria:

* invalid transitions rejected

---

### **T030 — Implement transition endpoint**

POST /targets/{id}/transition

Dependencies: T029

Acceptance Criteria:

* state updates correctly

---

### **T031 — Emit transition events**

Publish TargetTransitioned.

Dependencies: T009

Acceptance Criteria:

* event visible on bus

---

# 

# **Epic 6 — Asset Registry**

### **T032 — Create asset service scaffold**

Initialize service.

Dependencies: T002

Acceptance Criteria:

* service starts

---

### **T033 — Implement asset creation endpoint**

POST /assets

Dependencies: T032

Acceptance Criteria:

* asset stored

---

### **T034 — Implement asset telemetry updates**

POST /assets/{id}/telemetry

Dependencies: T033

Acceptance Criteria:

* location updates

---

### **T035 — Implement asset query endpoint**

GET /assets

Dependencies: T033

Acceptance Criteria:

* returns assets

---

### **T036 — Emit asset update events**

Publish AssetUpdated.

Dependencies: T009

Acceptance Criteria:

* events emitted

---

# 

# **Epic 7 — Ingest Pipeline**

### **T037 — Create ingest service scaffold**

Initialize service.

Dependencies: T002

Acceptance Criteria:

* service runs

---

### **T038 — Implement detection ingest endpoint**

POST /ingest/detections

Dependencies: T037

Acceptance Criteria:

* detection stored

---

### **T039 — Emit detection events**

Publish DetectionCreated.

Dependencies: T009

Acceptance Criteria:

* events appear on bus

---

### **T040 — Implement FMV replay adapter**

Load detections from sample video.

Dependencies: T038

Acceptance Criteria:

* detections generated

---

# **Epic 8 — Fusion Engine**

### **T041 — Create fusion service scaffold**

Initialize service.

Dependencies: T002

Acceptance Criteria:

* service starts

---

### **T042 — Implement candidate clustering**

Group nearby detections.

Dependencies: T041

Acceptance Criteria:

* candidates produced

---

### **T043 — Emit fused candidate events**

Publish CandidateCreated.

Dependencies: T009

Acceptance Criteria:

* event emitted

---

# 

# **Epic 9 — Recommendation Engine**

### **T044 — Create recommendation service scaffold**

Initialize service.

Dependencies: T002

Acceptance Criteria:

* service runs

---

### **T045 — Implement scoring algorithm**

Rank assets.

Dependencies: T033, T025

Acceptance Criteria:

* ranked output produced

---

### **T046 — Implement recommendation endpoint**

POST /targets/{id}/recommend-assets

Dependencies: T045

Acceptance Criteria:

* response returned

---

### **T047 — Implement explainability output**

Expose score components.

Dependencies: T045

Acceptance Criteria:

* explanation returned

---

### **T048 — Emit recommendation event**

Publish RecommendationGenerated.

Dependencies: T009

Acceptance Criteria:

* event observed

---

# 

# **Epic 10 — Planning Service**

### **T049 — Create planning service scaffold**

Dependencies: T002

Acceptance Criteria:

* service runs

---

### **T050 — Implement route estimation**

Estimate path between asset and target.

Dependencies: T049

Acceptance Criteria:

* geometry returned

---

### **T051 — Implement task proposal endpoint**

POST /targets/{id}/propose-task

Dependencies: T049

Acceptance Criteria:

* task created

---

### **T052 — Emit task proposed event**

Publish TaskProposed.

Dependencies: T009

Acceptance Criteria:

* event emitted

---

# 

# **Epic 11 — Approval Workflow**

### **T053 — Implement approval endpoint**

POST /tasks/{id}/approve

Dependencies: T051

Acceptance Criteria:

* status updated

---

### **T054 — Implement rejection endpoint**

POST /tasks/{id}/reject

Dependencies: T053

Acceptance Criteria:

* status updated

---

### **T055 — Emit approval events**

Publish TaskApproved.

Dependencies: T009

Acceptance Criteria:

* event visible

---

# 

# **Epic 12 — Execution Adapter**

### **T056 — Create execution adapter service**

Dependencies: T002

Acceptance Criteria:

* service runs

---

### **T057 — Implement simulated execution**

Execute after delay.

Dependencies: T056

Acceptance Criteria:

* execution event generated

---

### **T058 — Emit execution event**

Publish TaskExecuted.

Dependencies: T009

Acceptance Criteria:

* event visible

---

# 

# **Epic 13 — Assessment**

### **T059 — Create assessment service scaffold**

Dependencies: T002

Acceptance Criteria:

* service starts

---

### **T060 — Implement assessment endpoint**

POST /tasks/{id}/assess

Dependencies: T059

Acceptance Criteria:

* assessment stored

---

### **T061 — Update target status on assessment**

Dependencies: T060

Acceptance Criteria:

* status becomes COMPLETE

---

### **T062 — Emit assessment event**

Dependencies: T009

Acceptance Criteria:

* event emitted

---

# 

# **Epic 14 — API Gateway**

### **T063 — Create gateway service**

Dependencies: T002

Acceptance Criteria:

* service runs

---

### **T064 — Implement service routing**

Dependencies: T063

Acceptance Criteria:

* gateway proxies requests

---

### **T065 — Implement authentication**

Dependencies: T063

Acceptance Criteria:

* JWT validation works

---

### **T066 — Implement WebSocket updates**

Dependencies: T063

Acceptance Criteria:

* clients receive events

---

# 

# **Epic 15 — Frontend**

### **T067 — Implement map UI**

Dependencies: T003

Acceptance Criteria:

* map renders

---

### **T068 — Render detections**

Dependencies: T038

Acceptance Criteria:

* markers appear

---

### **T069 — Implement workflow board UI**

Dependencies: T025

Acceptance Criteria:

* columns visible

---

### **T070 — Implement target detail panel**

Dependencies: T027

Acceptance Criteria:

* target metadata displayed

---

### **T071 — Implement asset recommendation UI**

Dependencies: T046

Acceptance Criteria:

* ranked assets shown

---

### **T072 — Implement planning workspace**

Dependencies: T051

Acceptance Criteria:

* engagement line drawn

---

### **T073 — Implement approval controls**

Dependencies: T053

Acceptance Criteria:

* approve/reject works

---

### **T074 — Implement execution status UI**

Dependencies: T058

Acceptance Criteria:

* task state updates

---

### **T075 — Implement BDA UI**

Dependencies: T060

Acceptance Criteria:

* analyst can submit assessment

---

# 

# **Epic 16 — Media Handling**

### **T076 — Implement media upload endpoint**

Dependencies: T010

Acceptance Criteria:

* files stored

---

### **T077 — Implement media retrieval endpoint**

Dependencies: T076

Acceptance Criteria:

* media accessible

---

# 

# **Epic 17 — Observability**

### **T078 — Implement structured logging**

Dependencies: T024

Acceptance Criteria:

* logs structured

---

### **T079 — Implement Prometheus metrics**

Dependencies: T024

Acceptance Criteria:

* metrics endpoint exists

---

### **T080 — Implement tracing**

Dependencies: T024

Acceptance Criteria:

* distributed traces visible

---

# 

# **Final Deliverable Definition**

System is considered **MVP complete** when:

1. A detection can be ingested.

2. Detection can be nominated to target.

3. Target appears on workflow board.

4. Assets can be recommended.

5. Task can be proposed.

6. Task can be approved.

7. Execution is simulated.

8. Assessment can close the target.

