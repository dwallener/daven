Here’s the **build-order DAG** in practical form: not every dependency edge, but the **implementation sequence that minimizes dead ends** and keeps the system demoable early.

The key principle is simple:

**Build the spine first.**

Everything depends on:

storage \+ domain models \+ workflow transitions

If that part is mush, the rest is decorative suffering.

---

# **1\. Critical path**

This is the shortest path to a working MVP:

T001 → T002 → T004 → T005  
T001 → T003  
T001 → T006 → T007 → T008  
T004 → T009  
T006 → T010

T004 → T011 → T012 → T014 → T015  
T004 → T013

T008 → T016 → T017 → T018 → T022 → T023  
T017 → T020 → T021  
T017 → T019

T002 → T024 → T025 → T027 → T029 → T030 → T031  
T016 \+ T025 → T026  
T022 → T028

T002 → T032 → T033 → T034 → T035 → T036

T002 → T044 → T045 → T046 → T047 → T048

T002 → T049 → T050 → T051 → T052

T051 → T053 → T054 → T055

T002 → T056 → T057 → T058

T002 → T059 → T060 → T061 → T062

T002 → T063 → T064 → T065 → T066

T003 → T067 → T069 → T070  
T038 → T068  
T046 → T071  
T051 → T072  
T053 → T073  
T058 → T074  
T060 → T075

T010 → T076 → T077

T024 → T078 → T079 → T080

That’s the bones. Below is the sane execution order.

---

# **2\. Recommended implementation waves**

## **Wave 0 — Repo and infrastructure foundation**

Do these first, in this order:

* **T001** Initialize repository structure

* **T002** Initialize Rust workspace

* **T003** Initialize frontend app

* **T004** Setup shared libraries folder

* **T005** Add common config loader

* **T006** Create docker-compose environment

* **T007** Setup Postgres with PostGIS

* **T008** Create migration framework

* **T009** Create NATS messaging client

* **T010** Setup object storage

Why this wave first:

* all later work needs runtime, config, and persistence

* avoids every service inventing its own weird little config ritual

---

## **Wave 1 — Domain and schema**

Next, define the nouns before building verbs.

* **T011** Implement Detection struct

* **T012** Implement Target struct

* **T013** Implement Asset struct

* **T014** Implement Task struct

* **T015** Implement Assessment struct

Then schema:

* **T016** Create detections table

* **T017** Create targets table

* **T018** Create target\_state\_history table

* **T019** Create assets table

* **T020** Create tasks table

* **T021** Create assessments table

* **T022** Create boards table

* **T023** Create audit\_events table

This wave is mandatory before meaningful service logic.

---

## **Wave 2 — Workflow spine**

This is the first true product slice. Do not skip ahead.

* **T024** Create workflow service scaffold

* **T025** Implement target creation endpoint

* **T027** Implement target retrieval endpoint

* **T029** Implement lifecycle transition logic

* **T030** Implement transition endpoint

* **T031** Emit transition events

* **T022 dependency already done**

* **T028** Implement board retrieval endpoint

* **T016 \+ T025**

* **T026** Implement nomination endpoint

At the end of Wave 2, you should be able to do:

Detection → Nominate → Target → Transition → Board view

If that does not work end-to-end, stop. Fix it before touching recommendation or planning.

---

# **3\. Parallelizable branches after the spine**

Once Wave 2 is stable, split work into three branches:

## **Branch A — Assets**

* **T032** Create asset service scaffold

* **T033** Implement asset creation endpoint

* **T034** Implement asset telemetry updates

* **T035** Implement asset query endpoint

* **T036** Emit asset update events

## **Branch B — Ingest**

* **T037** Create ingest service scaffold

* **T038** Implement detection ingest endpoint

* **T039** Emit detection events

* **T040** Implement FMV replay adapter

## **Branch C — Frontend baseline**

* **T067** Implement map UI

* **T069** Implement workflow board UI

* **T070** Implement target detail panel

These can proceed in parallel because they hang off the spine, not each other.

---

# **4\. Next dependency layer**

After assets and workflow exist, do recommendation.

## **Recommendation layer**

* **T044** Create recommendation service scaffold

* **T045** Implement scoring algorithm

* **T046** Implement recommendation endpoint

* **T047** Implement explainability output

* **T048** Emit recommendation event

Hard dependencies:

* workflow target exists

* assets exist

So:

T025/T027/T030 \+ T033/T035 → T045 → T046

Frontend for this comes after the endpoint is real:

* **T071** Implement asset recommendation UI

---

# **5\. Planning and approval layer**

Once recommendation works, build tasking.

* **T049** Create planning service scaffold

* **T050** Implement route estimation

* **T051** Implement task proposal endpoint

* **T052** Emit task proposed event

Then approval:

* **T053** Implement approval endpoint

* **T054** Implement rejection endpoint

* **T055** Emit approval events

Frontend:

* **T072** Implement planning workspace

* **T073** Implement approval controls

Dependency chain:

Target \+ Asset \+ Recommendation → Task Proposal → Approval

No need to wait for execution or BDA.

---

# **6\. Execution and assessment layer**

Only after tasks can be approved should you build execution.

* **T056** Create execution adapter service

* **T057** Implement simulated execution

* **T058** Emit execution event

Then assessment:

* **T059** Create assessment service scaffold

* **T060** Implement assessment endpoint

* **T061** Update target status on assessment

* **T062** Emit assessment event

Frontend:

* **T074** Implement execution status UI

* **T075** Implement BDA UI

Dependency chain:

Approved Task → Simulated Execution → Assessment → Target Closure  
---

# **7\. Gateway and realtime timing**

The gateway can be brought up earlier, but it is smarter to do it once 2–3 backend services are real.

Recommended point: after workflow \+ asset \+ ingest baseline.

Do in this order:

* **T063** Create gateway service

* **T064** Implement service routing

* **T065** Implement authentication

* **T066** Implement WebSocket updates

This avoids building a gateway for imaginary services.

---

# **8\. Media and observability timing**

These should not block the main flow.

## **Media**

Do after ingest and assessment exist:

* **T076** Implement media upload endpoint

* **T077** Implement media retrieval endpoint

## **Observability**

Can start earlier, but practical order is:

* **T078** Implement structured logging

* **T079** Implement Prometheus metrics

* **T080** Implement tracing

Logging can start as soon as workflow exists. Metrics and tracing can layer in later.

---

# **9\. Best exact build order for Codex**

If I were feeding this to an agent, I’d use this exact sequence:

## **Stage A — Foundation**

1. T001

2. T002

3. T003

4. T004

5. T005

6. T006

7. T007

8. T008

9. T009

10. T010

## **Stage B — Domain \+ DB**

11. T011

12. T012

13. T013

14. T014

15. T015

16. T016

17. T017

18. T018

19. T019

20. T020

21. T021

22. T022

23. T023

## **Stage C — Workflow spine**

24. T024

25. T025

26. T027

27. T029

28. T030

29. T031

30. T028

31. T026

## **Stage D — Asset branch**

32. T032

33. T033

34. T034

35. T035

36. T036

## **Stage E — Ingest branch**

37. T037

38. T038

39. T039

40. T040

## **Stage F — Frontend baseline**

41. T067

42. T069

43. T070

44. T068

## **Stage G — Recommendation**

45. T044

46. T045

47. T046

48. T047

49. T048

50. T071

## **Stage H — Planning and approval**

51. T049

52. T050

53. T051

54. T052

55. T072

56. T053

57. T054

58. T055

59. T073

## **Stage I — Execution and assessment**

60. T056

61. T057

62. T058

63. T074

64. T059

65. T060

66. T061

67. T062

68. T075

## **Stage J — Gateway**

69. T063

70. T064

71. T065

72. T066

## **Stage K — Media**

73. T076

74. T077

## **Stage L — Observability**

75. T078

76. T079

77. T080

## **Stage M — Fusion**

This one is actually optional for MVP, and I’d intentionally place it late:

78\. T041

79\. T042

80\. T043

That’s a cleaner truth than pretending fusion is needed before the system can function.

---

# **10\. Tickets that can run in parallel**

These can be assigned simultaneously after the relevant stage opens.

## **Parallel set 1**

After Stage C:

* T032–T036

* T037–T040

* T067–T070

## **Parallel set 2**

After assets \+ workflow exist:

* T044–T048

* T063–T064

## **Parallel set 3**

After task proposal exists:

* T053–T055

* T072

* T078

## **Parallel set 4**

After execution exists:

* T059–T062

* T074–T075

* T076–T077

---

# **11\. Blocking dependencies to watch carefully**

These are the “don’t be cute” blockers.

## **Blocker A**

**T026 Nomination** depends on:

* detections table existing

* target creation logic existing

So:

T016 \+ T025 → T026

## **Blocker B**

**T046 Recommendations** depends on:

* targets queryable

* assets queryable

So:

T027 \+ T035 → T046

## **Blocker C**

**T051 Task proposal** depends on:

* target exists

* asset exists

* route estimation exists

So:

T027 \+ T035 \+ T050 → T051

## **Blocker D**

**T053 Approval** depends on:

* tasks already exist

So:

T051 → T053

## **Blocker E**

**T057 Simulated execution** depends on:

* approved task lifecycle existing

So:

T053 → T057

## **Blocker F**

**T060 Assessment** depends on:

* task exists, preferably executed

So:

T057/58 → T060  
---

# **12\. Suggested milestone cuts**

## **Milestone 1 — Workflow spine**

Complete:

* T001–T031 excluding nonessential infra polish

Demo:

* manually seed detection

* nominate target

* move target across board

## **Milestone 2 — Operational pairing**

Complete:

* T032–T048

* baseline frontend

Demo:

* target appears

* assets visible

* recommendation generated

## **Milestone 3 — Tasking loop**

Complete:

* T049–T062

Demo:

* propose task

* approve

* simulated execute

* assess and close

## **Milestone 4 — Platform shell**

Complete:

* T063–T080

Demo:

* auth

* realtime updates

* media

* metrics/logging/tracing

---

# **13\. A compact DAG view**

Here’s the concise graph:

Foundation:  
T001 → T002 → T004 → T005  
T001 → T003  
T001 → T006 → T007 → T008  
T004 → T009  
T006 → T010

Domain/DB:  
T004 → T011,T012,T013,T014,T015  
T008 → T016 → T017 → T018  
T017 → T019,T020,T022,T023  
T020 → T021

Workflow spine:  
T002 → T024 → T025 → T027 → T029 → T030 → T031  
T016 \+ T025 → T026  
T022 → T028

Assets:  
T002 → T032 → T033 → T034,T035 → T036

Ingest:  
T002 → T037 → T038 → T039 → T040

Recommendation:  
T002 → T044  
T027 \+ T035 → T045 → T046 → T047 → T048

Planning/Approval:  
T002 → T049 → T050  
T027 \+ T035 \+ T050 → T051 → T052  
T051 → T053,T054 → T055

Execution/Assessment:  
T002 → T056 → T057 → T058  
T002 → T059 → T060 → T061 → T062  
T058 → T060

Gateway/UI:  
T002 → T063 → T064 → T065 → T066  
T003 → T067 → T069 → T070  
T038 → T068  
T046 → T071  
T051 → T072  
T053 → T073  
T058 → T074  
T060 → T075

Media/Observability:  
T010 → T076 → T077  
T024 → T078 → T079 → T080

Fusion:  
T002 → T041 → T042 → T043  
---

# **14\. Practical advice for Codex execution**

When handing this to an agent, do not say “build the whole platform.” That’s how you get 1,800 lines of decorative nonsense and three broken Dockerfiles.

Feed it in milestone chunks:

1. **Foundation \+ schema**

2. **Workflow spine**

3. **Assets \+ ingest \+ map/board**

4. **Recommendation**

5. **Planning \+ approval**

6. **Execution \+ assessment**

7. **Gateway \+ realtime**

8. **Media \+ observability**

That sequencing keeps the system continuously runnable.

