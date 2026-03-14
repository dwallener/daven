export function App() {
  return (
    <main className="app-shell">
      <section className="hero">
        <p className="eyebrow">Daven MVP</p>
        <h1>Detection to action, built around the workflow spine.</h1>
        <p className="lede">
          Frontend scaffolding is in place. The next step is wiring the map,
          workflow board, and target detail panel onto the backend contracts.
        </p>
      </section>
      <section className="status-grid">
        <article>
          <h2>Workflow Service</h2>
          <p>Target creation, nomination, transitions, and board views.</p>
        </article>
        <article>
          <h2>Shared Models</h2>
          <p>Canonical domain types and event envelopes for service contracts.</p>
        </article>
        <article>
          <h2>Infrastructure</h2>
          <p>Postgres/PostGIS, NATS, Redis, and MinIO are stubbed for local dev.</p>
        </article>
      </section>
    </main>
  );
}
