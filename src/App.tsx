import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { FarmCanvas } from "./components/FarmCanvas";
import { Inspector } from "./components/Inspector";
import { RobotWorker } from "./components/RobotWorker";
import { WorkerList } from "./components/WorkerList";
import {
  copyResumeCommand,
  getSnapshot,
  isWindowFocused,
  loadSettings,
  markReviewed,
  openThread,
  sendNativeNotification,
  subscribeToSnapshots,
} from "./lib/bridge";
import { plotPosition, workerPosition } from "./lib/layout";
import { attentionFor, groupWorkers } from "./lib/workers";
import type { MonitorSnapshot, PlowSettings, RepoPlot, Worker } from "./types";
import "./styles.css";

function EmptyFarm({ connected }: { connected: boolean }) {
  return (
    <div className="empty-farm">
      <img src="/assets/plow-worker-v2.png" alt="A smiling robot farmer" />
      <h2>{connected ? "The fields are quiet" : "Waking the farm…"}</h2>
      <p>{connected ? "Start a Codex session on the shared daemon and a worker will arrive." : "Plow is looking for the local Codex daemon."}</p>
      {connected && <code>codex --remote unix://</code>}
    </div>
  );
}

export default function App() {
  const [snapshot, setSnapshot] = useState<MonitorSnapshot | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [listOpen, setListOpen] = useState(false);
  const [settings, setSettings] = useState<PlowSettings | null>(null);
  const notifiedRef = useRef(new Set<string>());

  useEffect(() => {
    void getSnapshot().then(setSnapshot);
    void loadSettings().then(setSettings);
    let unlisten: () => void = () => undefined;
    void subscribeToSnapshots((next) => setSnapshot(next)).then((fn) => { unlisten = fn; });
    return () => unlisten();
  }, []);

  useEffect(() => {
    if (!snapshot || !settings?.notifyWhenUnfocused) return;
    const attentions = snapshot.workers.map(attentionFor).filter((item) => item !== null);
    for (const attention of attentions) {
      if (notifiedRef.current.has(attention.key)) continue;
      notifiedRef.current.add(attention.key);
      void isWindowFocused().then((focused) => {
        if (!focused) void sendNativeNotification(attention.title, attention.body);
      });
    }
  }, [snapshot, settings]);

  const plots = useMemo(() => groupWorkers(snapshot?.workers ?? []), [snapshot?.workers]);
  const selected = snapshot?.workers.find((worker) => worker.id === selectedId) ?? null;
  const attentionCount = snapshot?.workers.filter((worker) => worker.status !== "running").length ?? 0;
  const connected = snapshot?.connection.status === "connected";

  const removeReviewed = async (worker: Worker) => {
    if (!worker.attentionId) return;
    await markReviewed(worker.attentionId);
    setSnapshot((current) => current ? { ...current, workers: current.workers.filter((item) => item.attentionId !== worker.attentionId) } : current);
    setSelectedId(null);
  };

  return (
    <main className={`${settings?.reducedMotion ? "app app--reduced-motion" : "app"}${(snapshot?.workers.length ?? 0) > 50 ? " app--crowded" : (snapshot?.workers.length ?? 0) > 20 ? " app--dense" : ""}`}>
      <header className="topbar">
        <div className="brand">
          <img src="/assets/plow-bot.png" alt="" />
          <div><h1>Plow</h1><p>Codex farm monitor</p></div>
        </div>
        <div className="topbar__actions">
          {attentionCount > 0 && <span className="attention-pill"><strong>{attentionCount}</strong> need attention</span>}
          <button type="button" className="button button--glass" onClick={() => setListOpen((open) => !open)} aria-expanded={listOpen}>Workers <span>{snapshot?.workers.length ?? 0}</span></button>
          <div className={`connection connection--${snapshot?.connection.status ?? "connecting"}`} title={snapshot?.connection.message}>
            <span />{connected ? "Connected" : snapshot?.connection.status === "missingCodex" ? "Codex missing" : "Connecting"}
          </div>
        </div>
      </header>

      <section className="farm" aria-label="Codex agent farm">
        <FarmCanvas />
        <div className="farm__wash" />
        {plots.map((plot) => (
          <FarmPlot
            key={plot.id}
            plot={plot}
            selectedId={selectedId}
            onSelect={(worker) => setSelectedId(worker.id)}
          />
        ))}
        {snapshot && snapshot.workers.length === 0 && <EmptyFarm connected={connected} />}
        {!snapshot && <EmptyFarm connected={false} />}
      </section>

      {listOpen && <WorkerList plots={plots} selectedId={selectedId} onSelect={(worker) => { setSelectedId(worker.id); setListOpen(false); }} />}
      <Inspector
        worker={selected}
        onClose={() => setSelectedId(null)}
        onOpen={async (worker) => { await openThread(worker.id, worker.cwd); }}
        onReviewed={removeReviewed}
        onCopy={async (worker) => { await copyResumeCommand(worker.id, worker.cwd); }}
      />
      <footer className="farm-footer">
        <span>{plots.length} field{plots.length === 1 ? "" : "s"}</span>
        <span className="farm-footer__hint">Select a worker to inspect their thread</span>
        <span>{snapshot?.connection.message ?? "Starting Plow"}</span>
      </footer>
    </main>
  );
}

function FarmPlot({ plot, selectedId, onSelect }: { plot: RepoPlot; selectedId: string | null; onSelect: (worker: Worker) => void }) {
  const anchor = plotPosition(plot.id);
  return (
    <Fragment>
      <div className="farm-plot-label" style={{ left: `${anchor.x}%`, top: `${anchor.y - 17}%` }}>{plot.name}</div>
      {plot.workers.map((worker) => {
        const position = workerPosition(plot.id, worker.id);
        return (
          <RobotWorker
            key={worker.id}
            worker={worker}
            selected={worker.id === selectedId}
            x={position.x}
            y={position.y}
            onSelect={onSelect}
          />
        );
      })}
    </Fragment>
  );
}
