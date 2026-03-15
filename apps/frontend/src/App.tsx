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

const workflowApiUrl =
  import.meta.env.VITE_WORKFLOW_API_URL ?? "http://127.0.0.1:3003";
const assetApiUrl =
  import.meta.env.VITE_ASSET_API_URL ?? "http://127.0.0.1:3004";
const recommendationApiUrl =
  import.meta.env.VITE_RECOMMENDATION_API_URL ?? "http://127.0.0.1:3005";
const planningApiUrl =
  import.meta.env.VITE_PLANNING_API_URL ?? "http://127.0.0.1:3006";

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

export function App() {
  const [boardData, setBoardData] = useState<BoardTargetsResponse | null>(null);
  const [assets, setAssets] = useState<Asset[]>([]);
  const [selectedTargetId, setSelectedTargetId] = useState<string | null>(null);
  const [recommendation, setRecommendation] = useState<Recommendation | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [boardError, setBoardError] = useState<string | null>(null);
  const [assetError, setAssetError] = useState<string | null>(null);
  const [recommendationError, setRecommendationError] = useState<string | null>(null);
  const [taskError, setTaskError] = useState<string | null>(null);
  const [loadingBoard, setLoadingBoard] = useState(true);
  const [loadingAssets, setLoadingAssets] = useState(true);
  const [loadingRecommendation, setLoadingRecommendation] = useState(false);
  const [loadingTasks, setLoadingTasks] = useState(false);
  const [submittingTask, setSubmittingTask] = useState(false);

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

  useEffect(() => {
    void loadBoard();
    void loadAssets();
  }, []);

  useEffect(() => {
    if (!selectedTargetId) {
      setRecommendation(null);
      setTasks([]);
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
  }, [selectedTargetId]);

  const allTargets = boardData ? Object.values(boardData.columns).flat() : [];
  const selectedTarget =
    allTargets.find((target) => target.id === selectedTargetId) ?? null;
  const assetMap = new Map(assets.map((asset) => [asset.id, asset]));
  const currentTask = tasks[0] ?? null;

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
      <section className="hero-panel">
        <div>
          <p className="eyebrow">Daven Operations Board</p>
          <h1>Workflow, pairing, and asset context on one live screen.</h1>
          <p className="lede">
            This board is reading the workflow, asset, and recommendation APIs
            directly. The current MVP path is detection to nomination to ranked
            asset suggestions.
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
        </dl>
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
            </header>
            {selectedTarget ? (
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
            ) : (
              <p className="empty-text">Pick a target card to inspect its live details.</p>
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
                      submittingTask || currentTask.approval_status === "APPROVED"
                    }
                    onClick={() => void updateTaskApproval("approve")}
                    type="button"
                  >
                    Approve Task
                  </button>
                  <button
                    className="action-button secondary"
                    disabled={
                      submittingTask || currentTask.approval_status === "REJECTED"
                    }
                    onClick={() => void updateTaskApproval("reject")}
                    type="button"
                  >
                    Reject Task
                  </button>
                </div>
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
