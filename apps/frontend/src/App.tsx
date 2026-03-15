import { startTransition, useEffect, useState } from "react";

type PointGeometry = {
  type: "Point";
  coordinates: [number, number];
};

type TargetStateTransition = {
  from: string | null;
  to: string;
  at: string;
  by: string;
};

type Target = {
  id: string;
  board_id: string;
  title: string;
  status: string;
  classification: string | null;
  priority: number;
  location: PointGeometry;
  source_detection_id: string | null;
  created_by: string;
  created_at: string;
  updated_at: string;
  labels: string[];
  state_history: TargetStateTransition[];
};

type Board = {
  id: string;
  name: string;
  statuses: string[];
};

type BoardTargetsResponse = {
  board: Board;
  columns: Record<string, Target[]>;
};

type Asset = {
  id: string;
  callsign: string;
  platform_type: string;
  domain: string;
  availability: string;
  capabilities: string[];
  updated_at: string;
  location: PointGeometry;
};

type RecommendationCandidate = {
  asset_id: string;
  score: number;
  rank: number;
  explanation: {
    availability: string;
    capability_match: number;
    distance_km: number;
  };
};

type Recommendation = {
  id: string;
  target_id: string;
  generated_at: string;
  candidates: RecommendationCandidate[];
};

type Task = {
  id: string;
  target_id: string;
  asset_ids: string[];
  task_type: string;
  effect_type: string;
  status: string;
  approval_status: string;
  time_on_target: string | null;
};

type Assessment = {
  id: string;
  task_id: string;
  target_id: string;
  result: "DESTROYED" | "DAMAGED" | "NO_EFFECT" | "INCONCLUSIVE";
  confidence: number;
  assessed_by: string;
  created_at: string;
  notes: string | null;
  media_refs: string[];
};

type ServiceHealth = {
  label: string;
  url: string;
  status: "checking" | "up" | "down";
};

const workflowApiUrl =
  import.meta.env.VITE_WORKFLOW_API_URL ?? "http://127.0.0.1:3003";
const assetApiUrl =
  import.meta.env.VITE_ASSET_API_URL ?? "http://127.0.0.1:3004";
const recommendationApiUrl =
  import.meta.env.VITE_RECOMMENDATION_API_URL ?? "http://127.0.0.1:3005";
const planningApiUrl =
  import.meta.env.VITE_PLANNING_API_URL ?? "http://127.0.0.1:3006";
const executionApiUrl =
  import.meta.env.VITE_EXECUTION_API_URL ?? "http://127.0.0.1:3007";
const assessmentApiUrl =
  import.meta.env.VITE_ASSESSMENT_API_URL ?? "http://127.0.0.1:3008";
const mapCenterLng = Number.parseFloat(
  import.meta.env.VITE_MAP_CENTER_LNG ?? "50.324",
);
const mapCenterLat = Number.parseFloat(
  import.meta.env.VITE_MAP_CENTER_LAT ?? "29.238",
);
const mapRadiusDegrees = Number.parseFloat(
  import.meta.env.VITE_MAP_RADIUS_DEGREES ?? "0.18",
);

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText}`);
  }
  return (await response.json()) as T;
}

function formatTimestamp(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function formatCoordinates(location: PointGeometry) {
  return `${location.coordinates[1].toFixed(3)}, ${location.coordinates[0].toFixed(3)}`;
}

function describeMissionPhase(target: Target | null) {
  if (!target) {
    return "No target selected";
  }

  switch (target.status) {
    case "NOMINATED":
    case "TRIAGED":
      return "Deliberation";
    case "PENDING_PAIRING":
      return "Pairing";
    case "PAIRED":
    case "PLAN_DRAFTED":
    case "PENDING_APPROVAL":
      return "Planning";
    case "APPROVED":
    case "IN_EXECUTION":
      return "Execution";
    case "PENDING_BDA":
      return "Assessment";
    case "ASSESSED_COMPLETE":
      return "Closed";
    default:
      return "Review";
  }
}

function describeNextAction(target: Target | null, task: Task | null) {
  if (!target) {
    return "Select a target to continue.";
  }

  if (!task) {
    return "Propose a task from the top recommendation.";
  }

  if (task.approval_status === "REQUIRED") {
    return "Approve or reject the proposed task.";
  }

  if (task.status === "APPROVED") {
    return "Dispatch the approved task.";
  }

  if (task.status === "IN_EXECUTION") {
    return "Mark execution complete when effects are observed.";
  }

  if (task.status === "COMPLETED" && target.status === "PENDING_BDA") {
    return "Submit BDA to close the target or keep it pending.";
  }

  if (target.status === "ASSESSED_COMPLETE") {
    return "Target closed. Review the timeline and evidence.";
  }

  return "Review target state and continue the workflow.";
}

function projectToMap(location: PointGeometry) {
  const [lng, lat] = location.coordinates;
  const x = ((lng - mapCenterLng) / mapRadiusDegrees + 1) * 50;
  const y = (1 - (lat - mapCenterLat) / mapRadiusDegrees) * 50;

  return {
    x: Math.min(96, Math.max(4, x)),
    y: Math.min(96, Math.max(4, y)),
  };
}

function isWithinMapAoi(location: PointGeometry) {
  const [lng, lat] = location.coordinates;
  return (
    Math.abs(lng - mapCenterLng) <= mapRadiusDegrees &&
    Math.abs(lat - mapCenterLat) <= mapRadiusDegrees
  );
}

export function App() {
  const [boardData, setBoardData] = useState<BoardTargetsResponse | null>(null);
  const [assets, setAssets] = useState<Asset[]>([]);
  const [selectedTargetId, setSelectedTargetId] = useState<string | null>(null);
  const [recommendation, setRecommendation] = useState<Recommendation | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [assessments, setAssessments] = useState<Assessment[]>([]);
  const [boardError, setBoardError] = useState<string | null>(null);
  const [assetError, setAssetError] = useState<string | null>(null);
  const [recommendationError, setRecommendationError] = useState<string | null>(null);
  const [taskError, setTaskError] = useState<string | null>(null);
  const [assessmentError, setAssessmentError] = useState<string | null>(null);
  const [loadingBoard, setLoadingBoard] = useState(true);
  const [loadingAssets, setLoadingAssets] = useState(true);
  const [loadingRecommendation, setLoadingRecommendation] = useState(false);
  const [loadingTasks, setLoadingTasks] = useState(false);
  const [loadingAssessments, setLoadingAssessments] = useState(false);
  const [submittingTask, setSubmittingTask] = useState(false);
  const [serviceHealth, setServiceHealth] = useState<ServiceHealth[]>([
    { label: "Workflow", url: workflowApiUrl, status: "checking" },
    { label: "Assets", url: assetApiUrl, status: "checking" },
    { label: "Recommend", url: recommendationApiUrl, status: "checking" },
    { label: "Planning", url: planningApiUrl, status: "checking" },
    { label: "Execution", url: executionApiUrl, status: "checking" },
    { label: "Assess", url: assessmentApiUrl, status: "checking" },
  ]);
  const [assessmentResult, setAssessmentResult] =
    useState<Assessment["result"]>("DESTROYED");
  const [assessmentConfidence, setAssessmentConfidence] = useState("0.92");
  const [assessmentNotes, setAssessmentNotes] = useState("");
  const [assessmentMediaRefs, setAssessmentMediaRefs] = useState("clip_001");

  async function loadBoard() {
    setLoadingBoard(true);
    setBoardError(null);
    try {
      const data = await fetchJson<BoardTargetsResponse>(
        `${workflowApiUrl}/boards/dynamic-main/targets`,
      );
      setBoardData(data);

      const firstTarget =
        data.columns.DELIBERATE?.[0] ?? Object.values(data.columns).flat()[0] ?? null;

      startTransition(() => {
        setSelectedTargetId((current) => current ?? firstTarget?.id ?? null);
      });
    } catch (error) {
      setBoardError(error instanceof Error ? error.message : "Board request failed");
    } finally {
      setLoadingBoard(false);
    }
  }

  async function loadAssets() {
    setLoadingAssets(true);
    setAssetError(null);
    try {
      const data = await fetchJson<Asset[]>(`${assetApiUrl}/assets`);
      setAssets(data);
    } catch (error) {
      setAssetError(error instanceof Error ? error.message : "Asset request failed");
    } finally {
      setLoadingAssets(false);
    }
  }

  async function loadTasks(targetId: string) {
    setLoadingTasks(true);
    setTaskError(null);
    try {
      const data = await fetchJson<Task[]>(`${planningApiUrl}/tasks/targets/${targetId}`);
      setTasks(data);
    } catch (error) {
      setTaskError(error instanceof Error ? error.message : "Task request failed");
    } finally {
      setLoadingTasks(false);
    }
  }

  async function loadAssessments(targetId: string) {
    setLoadingAssessments(true);
    setAssessmentError(null);
    try {
      const data = await fetchJson<Assessment[]>(
        `${assessmentApiUrl}/targets/${targetId}/assessments`,
      );
      setAssessments(data);
    } catch (error) {
      setAssessmentError(
        error instanceof Error ? error.message : "Assessment request failed",
      );
    } finally {
      setLoadingAssessments(false);
    }
  }

  useEffect(() => {
    void loadBoard();
    void loadAssets();
  }, []);

  useEffect(() => {
    let cancelled = false;

    const pollHealth = async () => {
      const next = await Promise.all(
        serviceHealth.map(async ({ label, url }) => {
          try {
            const response = await fetch(`${url}/health`);
            return {
              label,
              url,
              status: response.ok ? "up" : "down",
            } as ServiceHealth;
          } catch {
            return {
              label,
              url,
              status: "down",
            } as ServiceHealth;
          }
        }),
      );

      if (!cancelled) {
        setServiceHealth(next);
      }
    };

    void pollHealth();
    const timer = window.setInterval(() => {
      void pollHealth();
    }, 5000);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    if (!selectedTargetId) {
      setRecommendation(null);
      setTasks([]);
      setAssessments([]);
      return;
    }

    const loadRecommendation = async () => {
      setLoadingRecommendation(true);
      setRecommendationError(null);
      try {
        const response = await fetchJson<{ resource: Recommendation }>(
          `${recommendationApiUrl}/recommendations/targets/${selectedTargetId}`,
        );
        setRecommendation(response.resource);
      } catch (error) {
        setRecommendationError(
          error instanceof Error ? error.message : "Recommendation request failed",
        );
      } finally {
        setLoadingRecommendation(false);
      }
    };

    void loadRecommendation();
    void loadTasks(selectedTargetId);
    void loadAssessments(selectedTargetId);
  }, [selectedTargetId]);

  const allTargets = boardData ? Object.values(boardData.columns).flat() : [];
  const selectedTarget =
    allTargets.find((target) => target.id === selectedTargetId) ?? null;
  const assetMap = new Map(assets.map((asset) => [asset.id, asset]));
  const currentTask = tasks[0] ?? null;
  const missionPhase = describeMissionPhase(selectedTarget);
  const nextAction = describeNextAction(selectedTarget, currentTask);
  const mapTargets = allTargets.map((target) => ({
    id: target.id,
    kind: "target" as const,
    label: target.title,
    highlighted: target.id === selectedTargetId,
    status: target.status,
    point: projectToMap(target.location),
    inAoi: isWithinMapAoi(target.location),
  }));
  const mapAssets = assets.slice(0, 8).map((asset) => ({
    id: asset.id,
    kind: "asset" as const,
    label: asset.callsign,
    highlighted: currentTask?.asset_ids.includes(asset.id) ?? false,
    status: asset.availability,
    point: projectToMap(asset.location),
    inAoi: isWithinMapAoi(asset.location),
  }));
  const visibleTargetCount = mapTargets.filter((item) => item.inAoi).length;
  const visibleAssetCount = mapAssets.filter((item) => item.inAoi).length;
  const hiddenTargetCount = mapTargets.length - visibleTargetCount;
  const hiddenAssetCount = mapAssets.length - visibleAssetCount;

  async function proposeTask() {
    if (!selectedTarget || !recommendation?.candidates[0]) {
      return;
    }

    setSubmittingTask(true);
    setTaskError(null);
    try {
      const response = await fetch(`${planningApiUrl}/tasks/propose`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          target_id: selectedTarget.id,
          asset_ids: [recommendation.candidates[0].asset_id],
          task_type: "SMACK",
          effect_type: "kinetic",
          created_by: "operator",
        }),
      });
      if (!response.ok) {
        throw new Error(`${response.status} ${response.statusText}`);
      }
      await loadTasks(selectedTarget.id);
      await loadBoard();
    } catch (error) {
      setTaskError(error instanceof Error ? error.message : "Task proposal failed");
    } finally {
      setSubmittingTask(false);
    }
  }

  async function updateTaskApproval(action: "approve" | "reject") {
    if (!currentTask) {
      return;
    }

    setSubmittingTask(true);
    setTaskError(null);
    try {
      const response = await fetch(`${planningApiUrl}/tasks/${currentTask.id}/${action}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ actor: "operator" }),
      });
      if (!response.ok) {
        throw new Error(`${response.status} ${response.statusText}`);
      }
      if (selectedTarget) {
        await loadTasks(selectedTarget.id);
        await loadAssessments(selectedTarget.id);
      }
      await loadBoard();
    } catch (error) {
      setTaskError(error instanceof Error ? error.message : `Task ${action} failed`);
    } finally {
      setSubmittingTask(false);
    }
  }

  async function submitAssessment() {
    if (!currentTask || !selectedTarget) {
      return;
    }

    setSubmittingTask(true);
    setAssessmentError(null);
    try {
      const confidence = Number.parseFloat(assessmentConfidence);
      const response = await fetch(`${assessmentApiUrl}/tasks/${currentTask.id}/assess`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          result: assessmentResult,
          confidence,
          notes: assessmentNotes.trim() || null,
          media_refs: assessmentMediaRefs
            .split(",")
            .map((value) => value.trim())
            .filter(Boolean),
          actor: "operator",
        }),
      });
      if (!response.ok) {
        throw new Error(`${response.status} ${response.statusText}`);
      }
      await loadAssessments(selectedTarget.id);
      await loadBoard();
    } catch (error) {
      setAssessmentError(
        error instanceof Error ? error.message : "Assessment submission failed",
      );
    } finally {
      setSubmittingTask(false);
    }
  }

  async function updateExecution(action: "dispatch" | "complete") {
    if (!currentTask) {
      return;
    }

    setSubmittingTask(true);
    setTaskError(null);
    try {
      const response = await fetch(`${executionApiUrl}/tasks/${currentTask.id}/${action}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          actor: "operator",
          notes: `ui ${action}`,
        }),
      });
      if (!response.ok) {
        throw new Error(`${response.status} ${response.statusText}`);
      }
      if (selectedTarget) {
        await loadTasks(selectedTarget.id);
      }
      await loadBoard();
    } catch (error) {
      setTaskError(error instanceof Error ? error.message : `Task ${action} failed`);
    } finally {
      setSubmittingTask(false);
    }
  }

  return (
    <main className="app-shell">
      <section className="ops-grid">
        <section className="detail-card map-panel">
          <header className="panel-header compact">
            <div>
              <p className="section-kicker">Operations Map</p>
              <h2>Kharg Island Overlay</h2>
            </div>
            <span className="status-pill subtle">{missionPhase}</span>
          </header>
          <div className="map-frame">
            <div className="map-backdrop" />
            <div className="map-ring map-ring-a" />
            <div className="map-ring map-ring-b" />
            <div className="map-crosshair map-crosshair-h" />
            <div className="map-crosshair map-crosshair-v" />
            <div className="map-coast map-coast-main" />
            <div className="map-coast map-coast-island" />
            <div className="map-center-label">
              Centered on Kharg Island
              <span>{mapCenterLat.toFixed(3)}, {mapCenterLng.toFixed(3)}</span>
            </div>
            <div className="map-overlay-stats">
              <span>{visibleTargetCount} targets in AOI</span>
              <span>{visibleAssetCount} assets in AOI</span>
            </div>
            {mapTargets.filter((item) => item.inAoi).map((item) => (
              <button
                className={`map-marker ${item.kind}${item.highlighted ? " highlighted" : ""}`}
                key={item.id}
                onClick={() => setSelectedTargetId(item.id)}
                style={{ left: `${item.point.x}%`, top: `${item.point.y}%` }}
                type="button"
              >
                <span>{item.label}</span>
              </button>
            ))}
            {mapAssets.filter((item) => item.inAoi).map((item) => (
              <div
                className={`map-marker ${item.kind}${item.highlighted ? " highlighted" : ""}`}
                key={item.id}
                style={{ left: `${item.point.x}%`, top: `${item.point.y}%` }}
              >
                <span>{item.label}</span>
              </div>
            ))}
            {hiddenTargetCount || hiddenAssetCount ? (
              <div className="map-offboard">
                <p className="detail-label">Outside Kharg AOI</p>
                <p>
                  {hiddenTargetCount} targets and {hiddenAssetCount} assets are outside the
                  current map window.
                </p>
              </div>
            ) : null}
          </div>
          <div className="map-legend">
            <span className="legend-item">
              <span className="legend-swatch target" />
              Target
            </span>
            <span className="legend-item">
              <span className="legend-swatch asset" />
              Asset
            </span>
            <span className="legend-item">
              <span className="legend-swatch focus" />
              Selected / assigned
            </span>
          </div>
        </section>

        <section className="detail-card briefing-panel">
          <header className="panel-header compact">
            <div>
              <p className="section-kicker">Mission Brief</p>
              <h2>{selectedTarget?.title ?? "Awaiting selection"}</h2>
            </div>
          </header>
          <div className="brief-grid">
            <div className="brief-block">
              <p className="detail-label">Phase</p>
              <p>{missionPhase}</p>
            </div>
            <div className="brief-block">
              <p className="detail-label">Priority</p>
              <p>{selectedTarget ? `P${selectedTarget.priority}` : "N/A"}</p>
            </div>
            <div className="brief-block brief-span">
              <p className="detail-label">Next Action</p>
              <p>{nextAction}</p>
            </div>
            <div className="brief-block brief-span">
              <p className="detail-label">Current Focus</p>
              <p>
                {selectedTarget
                  ? `${selectedTarget.classification ?? "Unknown"} target near ${formatCoordinates(
                      selectedTarget.location,
                    )}`
                  : "Choose a target on the board or map to inspect it."}
              </p>
            </div>
          </div>
        </section>
      </section>

      <section className="hero-panel compact-hero">
        <div>
          <p className="eyebrow">Daven Operations Board</p>
          <h1>Workflow, pairing, and asset context on one live screen.</h1>
          <p className="lede">
            This board is reading the workflow, asset, recommendation,
            planning, execution, and assessment APIs directly. The current
            MVP path runs from detection to nomination to pairing to approved
            task execution and BDA closeout.
          </p>
        </div>
        <dl className="endpoint-grid">
          <div>
            <dt>Workflow</dt>
            <dd>{workflowApiUrl}</dd>
          </div>
          <div>
            <dt>Assets</dt>
            <dd>{assetApiUrl}</dd>
          </div>
          <div>
            <dt>Recommendations</dt>
            <dd>{recommendationApiUrl}</dd>
          </div>
          <div>
            <dt>Planning</dt>
            <dd>{planningApiUrl}</dd>
          </div>
          <div>
            <dt>Execution</dt>
            <dd>{executionApiUrl}</dd>
          </div>
          <div>
            <dt>Assessment</dt>
            <dd>{assessmentApiUrl}</dd>
          </div>
        </dl>
        <div className="health-strip">
          {serviceHealth.map((service) => (
            <div className="health-chip" key={service.label}>
              <span className={`health-dot ${service.status}`} />
              <span>{service.label}</span>
            </div>
          ))}
        </div>
      </section>

      <section className="main-grid">
        <section className="board-panel">
          <header className="panel-header">
            <div>
              <p className="section-kicker">Workflow Board</p>
              <h2>Dynamic Main</h2>
            </div>
            <span className="status-pill">
              {loadingBoard ? "Loading board" : `${allTargets.length} targets`}
            </span>
          </header>
          {boardError ? <p className="error-text">{boardError}</p> : null}
          <div className="board-columns">
            {boardData?.board.statuses.map((columnName) => {
              const targets = boardData.columns[columnName] ?? [];
              return (
                <section className="board-column" key={columnName}>
                  <header>
                    <h3>{columnName.replaceAll("_", " ")}</h3>
                    <span>{targets.length}</span>
                  </header>
                  <div className="card-stack">
                    {targets.length === 0 ? (
                      <p className="empty-text">No targets in this lane.</p>
                    ) : null}
                    {targets.map((target) => {
                      const selected = target.id === selectedTargetId;
                      return (
                        <button
                          className={`target-card${selected ? " selected" : ""}`}
                          key={target.id}
                          onClick={() => setSelectedTargetId(target.id)}
                          type="button"
                        >
                          <div className="target-card-top">
                            <p>{target.title}</p>
                            <span>P{target.priority}</span>
                          </div>
                          <p className="target-meta">
                            {target.classification ?? "unclassified"}
                          </p>
                          <p className="target-meta">
                            {formatCoordinates(target.location)}
                          </p>
                          <div className="label-row">
                            {target.labels.map((label) => (
                              <span className="label-pill" key={label}>
                                {label}
                              </span>
                            ))}
                          </div>
                        </button>
                      );
                    })}
                  </div>
                </section>
              );
            })}
          </div>
        </section>

        <aside className="side-panel">
          <section className="detail-card">
            <header className="panel-header compact">
              <div>
                <p className="section-kicker">Selected Target</p>
                <h2>{selectedTarget?.title ?? "No target selected"}</h2>
              </div>
              <span className="status-pill subtle">{missionPhase}</span>
            </header>
            {selectedTarget ? (
              <>
                <div className="brief-banner">
                  <div>
                    <p className="detail-label">Next Action</p>
                    <p>{nextAction}</p>
                  </div>
                </div>
                <div className="detail-grid">
                  <div>
                  <p className="detail-label">Status</p>
                  <p>{selectedTarget.status}</p>
                  </div>
                  <div>
                  <p className="detail-label">Classification</p>
                  <p>{selectedTarget.classification ?? "Unknown"}</p>
                  </div>
                  <div>
                  <p className="detail-label">Location</p>
                  <p>{formatCoordinates(selectedTarget.location)}</p>
                  </div>
                  <div>
                  <p className="detail-label">Source Detection</p>
                  <p>{selectedTarget.source_detection_id ?? "Manual"}</p>
                  </div>
                  <div>
                  <p className="detail-label">Updated</p>
                  <p>{formatTimestamp(selectedTarget.updated_at)}</p>
                  </div>
                </div>
              </>
            ) : (
              <p className="empty-text">Pick a target card to inspect its live details.</p>
            )}
          </section>

          <section className="detail-card">
            <header className="panel-header compact">
              <div>
                <p className="section-kicker">Target Timeline</p>
                <h2>State Narrative</h2>
              </div>
              <span className="status-pill subtle">
                {selectedTarget?.state_history.length ?? 0}
              </span>
            </header>
            {selectedTarget?.state_history.length ? (
              <div className="timeline-list">
                {selectedTarget.state_history
                  .slice()
                  .reverse()
                  .map((entry, index) => (
                    <article className="timeline-item" key={`${entry.at}-${index}`}>
                      <div className="timeline-dot" />
                      <div>
                        <p className="timeline-title">
                          {entry.from ? `${entry.from} -> ${entry.to}` : entry.to}
                        </p>
                        <p className="target-meta">By {entry.by}</p>
                        <p className="target-meta">{formatTimestamp(entry.at)}</p>
                      </div>
                    </article>
                  ))}
              </div>
            ) : (
              <p className="empty-text">No target history is available yet.</p>
            )}
          </section>

          <section className="detail-card">
            <header className="panel-header compact">
              <div>
                <p className="section-kicker">Recommendation Stack</p>
                <h2>Candidate Assets</h2>
              </div>
              <span className="status-pill subtle">
                {loadingRecommendation ? "Scoring" : recommendation?.candidates.length ?? 0}
              </span>
            </header>
            {recommendationError ? <p className="error-text">{recommendationError}</p> : null}
            <div className="recommendation-list">
              {recommendation?.candidates.map((candidate) => {
                const asset = assetMap.get(candidate.asset_id);
                return (
                  <article className="recommendation-card" key={candidate.asset_id}>
                    <div className="target-card-top">
                      <p>{asset?.callsign ?? candidate.asset_id}</p>
                      <span>#{candidate.rank}</span>
                    </div>
                    <p className="target-meta">
                      {asset?.platform_type ?? "unknown"} · {asset?.domain ?? "unknown"}
                    </p>
                    <p className="score-line">
                      Score <strong>{candidate.score.toFixed(3)}</strong>
                    </p>
                    <p className="target-meta">
                      Distance {candidate.explanation.distance_km.toFixed(1)} km
                    </p>
                    <p className="target-meta">
                      Capability match {candidate.explanation.capability_match.toFixed(2)}
                    </p>
                  </article>
                );
              })}
              {!recommendation && !loadingRecommendation ? (
                <p className="empty-text">Select a target to generate recommendations.</p>
              ) : null}
            </div>
            <div className="action-row">
              <button
                className="action-button"
                disabled={!selectedTarget || !recommendation?.candidates.length || submittingTask}
                onClick={() => void proposeTask()}
                type="button"
              >
                {submittingTask ? "Submitting" : "Propose Task From Top Candidate"}
              </button>
            </div>
          </section>

          <section className="detail-card">
            <header className="panel-header compact">
              <div>
                <p className="section-kicker">Task Planning</p>
                <h2>Drafts and Approvals</h2>
              </div>
              <span className="status-pill subtle">
                {loadingTasks ? "Loading tasks" : tasks.length}
              </span>
            </header>
            {taskError ? <p className="error-text">{taskError}</p> : null}
            {currentTask ? (
              <div className="task-panel">
                <div className="detail-grid">
                  <div>
                    <p className="detail-label">Task</p>
                    <p>{currentTask.task_type}</p>
                  </div>
                  <div>
                    <p className="detail-label">Effect</p>
                    <p>{currentTask.effect_type}</p>
                  </div>
                  <div>
                    <p className="detail-label">Status</p>
                    <p>{currentTask.status}</p>
                  </div>
                  <div>
                    <p className="detail-label">Approval</p>
                    <p>{currentTask.approval_status}</p>
                  </div>
                </div>
                <div className="action-row">
                  <button
                    className="action-button"
                    disabled={
                      submittingTask ||
                      currentTask.approval_status === "APPROVED" ||
                      currentTask.approval_status === "REJECTED"
                    }
                    onClick={() => void updateTaskApproval("approve")}
                    type="button"
                  >
                    Approve Task
                  </button>
                  <button
                    className="action-button secondary"
                    disabled={
                      submittingTask ||
                      currentTask.approval_status === "REJECTED" ||
                      currentTask.approval_status === "APPROVED"
                    }
                    onClick={() => void updateTaskApproval("reject")}
                    type="button"
                  >
                    Reject Task
                  </button>
                  <button
                    className="action-button"
                    disabled={
                      submittingTask ||
                      currentTask.approval_status !== "APPROVED" ||
                      currentTask.status !== "APPROVED"
                    }
                    onClick={() => void updateExecution("dispatch")}
                    type="button"
                  >
                    Dispatch Task
                  </button>
                  <button
                    className="action-button secondary"
                    disabled={
                      submittingTask ||
                      currentTask.approval_status !== "APPROVED" ||
                      currentTask.status !== "IN_EXECUTION"
                    }
                    onClick={() => void updateExecution("complete")}
                    type="button"
                  >
                    Complete Task
                  </button>
                </div>
                {currentTask.status === "COMPLETED" ? (
                  <div className="assessment-panel">
                    <div className="panel-header compact">
                      <div>
                        <p className="section-kicker">Post-Action Assessment</p>
                        <h2>BDA Submission</h2>
                      </div>
                    </div>
                    {assessmentError ? (
                      <p className="error-text">{assessmentError}</p>
                    ) : null}
                    <div className="form-grid">
                      <label className="field">
                        <span className="detail-label">Result</span>
                        <select
                          value={assessmentResult}
                          onChange={(event) =>
                            setAssessmentResult(event.target.value as Assessment["result"])
                          }
                        >
                          <option value="DESTROYED">Destroyed</option>
                          <option value="DAMAGED">Damaged</option>
                          <option value="NO_EFFECT">No Effect</option>
                          <option value="INCONCLUSIVE">Inconclusive</option>
                        </select>
                      </label>
                      <label className="field">
                        <span className="detail-label">Confidence</span>
                        <input
                          max="1"
                          min="0"
                          onChange={(event) => setAssessmentConfidence(event.target.value)}
                          step="0.01"
                          type="number"
                          value={assessmentConfidence}
                        />
                      </label>
                      <label className="field form-span-2">
                        <span className="detail-label">Media Refs</span>
                        <input
                          onChange={(event) => setAssessmentMediaRefs(event.target.value)}
                          placeholder="clip_001, pass_2_frame_87"
                          type="text"
                          value={assessmentMediaRefs}
                        />
                      </label>
                      <label className="field form-span-2">
                        <span className="detail-label">Notes</span>
                        <textarea
                          onChange={(event) => setAssessmentNotes(event.target.value)}
                          placeholder="confirmed from follow-up sensor pass"
                          rows={3}
                          value={assessmentNotes}
                        />
                      </label>
                    </div>
                    <div className="action-row">
                      <button
                        className="action-button"
                        disabled={submittingTask || selectedTarget.status !== "PENDING_BDA"}
                        onClick={() => void submitAssessment()}
                        type="button"
                      >
                        Submit Assessment
                      </button>
                    </div>
                  </div>
                ) : null}
              </div>
            ) : (
              <p className="empty-text">
                No task has been proposed for the selected target yet.
              </p>
            )}
          </section>

          <section className="detail-card">
            <header className="panel-header compact">
              <div>
                <p className="section-kicker">Assessment History</p>
                <h2>BDA Records</h2>
              </div>
              <span className="status-pill subtle">
                {loadingAssessments ? "Loading BDA" : assessments.length}
              </span>
            </header>
            {assessmentError ? <p className="error-text">{assessmentError}</p> : null}
            <div className="recommendation-list">
              {assessments.map((assessment) => (
                <article className="recommendation-card" key={assessment.id}>
                  <div className="target-card-top">
                    <p>{assessment.result.replaceAll("_", " ")}</p>
                    <span>{assessment.confidence.toFixed(2)}</span>
                  </div>
                  <p className="target-meta">By {assessment.assessed_by}</p>
                  <p className="target-meta">{formatTimestamp(assessment.created_at)}</p>
                  {assessment.notes ? (
                    <p className="target-meta">{assessment.notes}</p>
                  ) : null}
                  {assessment.media_refs.length ? (
                    <div className="label-row">
                      {assessment.media_refs.map((mediaRef) => (
                        <span className="label-pill" key={mediaRef}>
                          {mediaRef}
                        </span>
                      ))}
                    </div>
                  ) : null}
                </article>
              ))}
              {!assessments.length && !loadingAssessments ? (
                <p className="empty-text">
                  No assessments recorded for the selected target yet.
                </p>
              ) : null}
            </div>
          </section>

          <section className="detail-card">
            <header className="panel-header compact">
              <div>
                <p className="section-kicker">Asset Registry</p>
                <h2>Ready Platforms</h2>
              </div>
              <span className="status-pill subtle">
                {loadingAssets ? "Loading assets" : assets.length}
              </span>
            </header>
            {assetError ? <p className="error-text">{assetError}</p> : null}
            <div className="asset-list">
              {assets.map((asset) => (
                <article className="asset-card" key={asset.id}>
                  <div className="target-card-top">
                    <p>{asset.callsign}</p>
                    <span>{asset.availability}</span>
                  </div>
                  <p className="target-meta">
                    {asset.platform_type} · {asset.domain}
                  </p>
                  <p className="target-meta">{formatCoordinates(asset.location)}</p>
                  <div className="label-row">
                    {asset.capabilities.map((capability) => (
                      <span className="label-pill" key={capability}>
                        {capability}
                      </span>
                    ))}
                  </div>
                </article>
              ))}
            </div>
          </section>
        </aside>
      </section>
    </main>
  );
}
