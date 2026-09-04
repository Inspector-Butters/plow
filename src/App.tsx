import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FarmCanvas } from "./components/FarmCanvas";
import { Inspector } from "./components/Inspector";
import { ProjectLauncher } from "./components/ProjectLauncher";
import { RobotWorker } from "./components/RobotWorker";
import { SettingsPanel } from "./components/SettingsPanel";
import { UpdatePrompt } from "./components/UpdatePrompt";
import { WorkerList } from "./components/WorkerList";
import {
  copyResumeCommand,
  getAppVersion,
  getSnapshot,
  isWindowFocused,
  isNativeApp,
  loadSettings,
  markReviewed,
  openThread,
  sendNativeNotification,
  subscribeToSnapshots,
  updateSettings,
} from "./lib/bridge";
import { plotPosition, workerPosition } from "./lib/layout";
import { checkForAppUpdate, dismissAppUpdate, installAppUpdate } from "./lib/updater";
import { attentionFor, groupWorkers } from "./lib/workers";
import type { AppUpdateInfo } from "./lib/updater";
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

function initialDemoUpdate(): AppUpdateInfo | null {
  if (!import.meta.env.DEV || isNativeApp() || !new URLSearchParams(window.location.search).has("update")) return null;
  return {
    currentVersion: "0.3.2",
    version: "0.3.3",
    date: null,
    notes: "Smoother workers, a sturdier harvest, and a few small fixes around the farm.",
  };
}

export default function App() {
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<MonitorSnapshot | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [listOpen, setListOpen] = useState(false);
  const [launcherOpen, setLauncherOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settings, setSettings] = useState<PlowSettings | null>(null);
  const [availableUpdate, setAvailableUpdate] = useState<AppUpdateInfo | null>(initialDemoUpdate);
  const notifiedRef = useRef(new Set<string>());

  useEffect(() => {
    void getAppVersion().then(setAppVersion).catch((error) => console.warn("Plow could not read its version", error));
    void getSnapshot().then(setSnapshot);
    void loadSettings().then(setSettings);
    let unlisten: () => void = () => undefined;
    void subscribeToSnapshots((next) => setSnapshot(next)).then((fn) => { unlisten = fn; });
    return () => unlisten();
  }, []);

  useEffect(() => {
    let mounted = true;
    const check = () => {
      void checkForAppUpdate()
        .then((update) => {
          if (mounted && update) setAvailableUpdate(update);
        })
        .catch((error) => console.warn("Plow could not check for updates", error));
    };
    const checkWhenVisible = () => {
      if (document.visibilityState === "visible") check();
    };
    check();
    const interval = window.setInterval(checkWhenVisible, 15 * 60 * 1000);
    window.addEventListener("focus", check);
    document.addEventListener("visibilitychange", checkWhenVisible);
    return () => {
      mounted = false;
      window.clearInterval(interval);
      window.removeEventListener("focus", check);
      document.removeEventListener("visibilitychange", checkWhenVisible);
    };
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
  const closeSettings = useCallback(() => setSettingsOpen(false), []);
  const connectionLabel = connected
    ? "Connected"
    : snapshot?.connection.status === "missingCodex"
      ? "Codex missing"
      : snapshot?.connection.status === "incompatible"
        ? "Codex incompatible"
        : snapshot?.connection.status === "disconnected"
          ? "Connection problem"
          : "Connecting";

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
          <div><h1>Plow {appVersion && <small className="brand__version" aria-label={`Version ${appVersion}`}>v{appVersion}</small>}</h1><p>Codex farm monitor</p></div>
        </div>
        <div className="topbar__actions">
          {attentionCount > 0 && <span className="attention-pill"><strong>{attentionCount}</strong> need attention</span>}
          <button type="button" className="button button--primary topbar__start" onClick={() => { setListOpen(false); setLauncherOpen(true); }} disabled={!settings}>Start agent</button>
          <button type="button" className="button button--glass" onClick={() => setListOpen((open) => !open)} aria-expanded={listOpen}>Workers <span>{snapshot?.workers.length ?? 0}</span></button>
          <button type="button" className="button button--glass" onClick={() => setSettingsOpen(true)} disabled={!settings}>Settings</button>
          <button type="button" className={`connection connection--${snapshot?.connection.status ?? "connecting"}`} title={snapshot?.connection.message} onClick={() => setSettingsOpen(true)}>
            <span />{connectionLabel}
          </button>
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
      {launcherOpen && settings && (
        <ProjectLauncher
          developmentHome={settings.developmentHome}
          onClose={() => setLauncherOpen(false)}
          onOpenSettings={() => { setLauncherOpen(false); setSettingsOpen(true); }}
        />
      )}
      <Inspector
        worker={selected}
        onClose={() => setSelectedId(null)}
        onOpen={async (worker) => { await openThread(worker.id, worker.cwd); }}
        onReviewed={removeReviewed}
        onCopy={async (worker) => { await copyResumeCommand(worker.id, worker.cwd); }}
      />
      {settingsOpen && settings && (
        <SettingsPanel
          settings={settings}
          connection={snapshot?.connection ?? null}
          onClose={closeSettings}
          onSave={async (next) => {
            await updateSettings(next);
            setSettings(next);
            closeSettings();
          }}
        />
      )}
      {availableUpdate && (
        <UpdatePrompt
          update={availableUpdate}
          onDismiss={() => {
            setAvailableUpdate(null);
            void dismissAppUpdate().catch((error) => console.warn("Plow could not close the update check", error));
          }}
          onInstall={installAppUpdate}
        />
      )}
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
