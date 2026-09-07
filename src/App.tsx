import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AttentionList } from "./components/AttentionList";
import { ClassicDashboard } from "./components/ClassicDashboard";
import { FarmCanvas } from "./components/FarmCanvas";
import { Inspector } from "./components/Inspector";
import { ProjectLauncher } from "./components/ProjectLauncher";
import { RobotWorker } from "./components/RobotWorker";
import { SettingsPanel } from "./components/SettingsPanel";
import { UpdatePrompt } from "./components/UpdatePrompt";
import { UsageLimits } from "./components/UsageLimits";
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
import { workerPosition } from "./lib/layout";
import { checkForAppUpdate, dismissAppUpdate, installAppUpdate } from "./lib/updater";
import { attentionFor, groupWorkers } from "./lib/workers";
import type { AppUpdateInfo } from "./lib/updater";
import type { AgentViewMode, ConnectionStatus, MonitorSnapshot, PlowSettings, ProjectLocation, RepoPlot, Worker } from "./types";
import "./styles.css";

function EmptyFarm({ connected }: { connected: boolean }) {
  return (
    <div className="empty-farm">
      <img src="/assets/plow-worker-v2.png" alt="A smiling robot farmer" />
      <h2>{connected ? "The fields are quiet" : "Waking the farm…"}</h2>
      <p>{connected ? "Start a Codex session on a monitored host and a worker will arrive." : "Plow is connecting to your Codex hosts."}</p>
    </div>
  );
}

function initialDemoUpdate(): AppUpdateInfo | null {
  if (!import.meta.env.DEV || isNativeApp() || !new URLSearchParams(window.location.search).has("update")) return null;
  return {
    currentVersion: "0.4.2",
    version: "0.4.3",
    date: null,
    notes: "Keep the limits bar focused on the standard Codex quota windows.",
  };
}

export default function App() {
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<MonitorSnapshot | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [listOpen, setListOpen] = useState(false);
  const [attentionOpen, setAttentionOpen] = useState(false);
  const [launcherOpen, setLauncherOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settings, setSettings] = useState<PlowSettings | null>(null);
  const [viewSaveError, setViewSaveError] = useState<string | null>(null);
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
  const classicWorkers = useMemo(() => plots.flatMap((plot) => plot.workers), [plots]);
  const selected = snapshot?.workers.find((worker) => worker.id === selectedId) ?? null;
  const attentionWorkers = useMemo(
    () => snapshot?.workers.filter((worker) => attentionFor(worker) !== null) ?? [],
    [snapshot?.workers],
  );
  const attentionCount = attentionWorkers.length;
  useEffect(() => {
    if (attentionCount === 0) setAttentionOpen(false);
  }, [attentionCount]);
  const connections = snapshot?.connections ?? [];
  const connectedCount = connections.filter((connection) => connection.status === "connected").length;
  const connected = connectedCount > 0;
  const connectionStatus: ConnectionStatus = connected
    ? "connected"
    : connections.some((connection) => connection.status === "incompatible")
      ? "incompatible"
      : connections.some((connection) => connection.status === "missingCodex")
        ? "missingCodex"
        : connections.some((connection) => connection.status === "disconnected")
          ? "disconnected"
          : "connecting";
  const connectionMessage = connections.length === 0
    ? "Starting Plow"
    : connections.length === 1
      ? connections[0].message
      : `${connectedCount} of ${connections.length} Codex hosts connected`;
  const closeSettings = useCallback(() => setSettingsOpen(false), []);
  const dismissFloating = useCallback(() => {
    setSelectedId(null);
    setListOpen(false);
    setAttentionOpen(false);
  }, []);
  const viewMode = settings?.viewMode ?? "field";
  const connectionLabel = connectedCount > 0 && connections.length > 1
    ? `${connectedCount}/${connections.length} connected`
    : connected
      ? "Connected"
      : connectionStatus === "missingCodex"
      ? "Codex missing"
      : connectionStatus === "incompatible"
        ? "Codex incompatible"
        : connectionStatus === "disconnected"
          ? "Connection problem"
          : "Connecting";
  const projectLocations = useMemo<ProjectLocation[]>(() => {
    if (!settings) return [];
    const locations: ProjectLocation[] = [];
    if (settings.localEnabled) {
      locations.push({ hostId: "local", hostLabel: "This computer", hostKind: "local", developmentHome: settings.developmentHome });
    }
    for (const host of settings.sshHosts) {
      if (host.enabled) locations.push({ hostId: `ssh:${host.alias}`, hostLabel: host.alias, hostKind: "ssh", developmentHome: host.developmentHome });
    }
    return locations;
  }, [settings]);

  const removeReviewed = async (worker: Worker) => {
    if (!worker.attentionId) return;
    await markReviewed(worker.attentionId);
    setSnapshot((current) => current ? { ...current, workers: current.workers.filter((item) => item.attentionId !== worker.attentionId) } : current);
    setSelectedId(null);
  };

  const changeViewMode = async (nextMode: AgentViewMode) => {
    if (!settings || settings.viewMode === nextMode) return;
    const previous = settings;
    const next = { ...settings, viewMode: nextMode };
    setViewSaveError(null);
    setSettings(next);
    setListOpen(false);
    setAttentionOpen(false);
    try {
      await updateSettings(next);
    } catch (error) {
      setSettings((current) => current?.viewMode === nextMode ? previous : current);
      setViewSaveError(error instanceof Error ? error.message : "Could not save the agent view");
    }
  };

  return (
    <main className={`${settings?.reducedMotion ? "app app--reduced-motion" : "app"}${viewMode === "classic" ? " app--classic" : ""}${(snapshot?.workers.length ?? 0) > 50 ? " app--crowded" : (snapshot?.workers.length ?? 0) > 20 ? " app--dense" : ""}`}>
      <header className="topbar">
        <div className="brand">
          <img src="/assets/plow-bot.png" alt="" />
          <div><h1>Plow {appVersion && <small className="brand__version" aria-label={`Version ${appVersion}`}>v{appVersion}</small>}</h1><p>Codex farm monitor</p></div>
        </div>
        <div className="topbar__actions">
          {attentionCount > 0 && (
            <button type="button" className="attention-pill" aria-expanded={attentionOpen} onClick={() => {
              const nextOpen = !attentionOpen;
              dismissFloating();
              setAttentionOpen(nextOpen);
            }}><strong>{attentionCount}</strong> need attention</button>
          )}
          <div className="view-switch" role="group" aria-label="Agent view">
            <button type="button" className={viewMode === "field" ? "is-active" : ""} aria-pressed={viewMode === "field"} disabled={!settings} onClick={() => void changeViewMode("field")}>Field</button>
            <button type="button" className={viewMode === "classic" ? "is-active" : ""} aria-pressed={viewMode === "classic"} disabled={!settings} onClick={() => void changeViewMode("classic")}>Classic</button>
          </div>
          {viewSaveError && <span className="view-switch__error" role="alert" title={viewSaveError}>View not saved</span>}
          <button type="button" className="button button--primary topbar__start" onClick={() => { dismissFloating(); setLauncherOpen(true); }} disabled={!settings}>Start agent</button>
          <button type="button" className="button button--glass" onClick={() => { const nextOpen = !listOpen; dismissFloating(); setListOpen(nextOpen); }} aria-expanded={listOpen}>Workers <span>{snapshot?.workers.length ?? 0}</span></button>
          <button type="button" className="button button--glass" onClick={() => { dismissFloating(); setSettingsOpen(true); }} disabled={!settings}>Settings</button>
          <button type="button" className={`connection connection--${connectionStatus}`} title={connectionMessage} onClick={() => { dismissFloating(); setSettingsOpen(true); }}>
            <span />{connectionLabel}
          </button>
        </div>
      </header>

      {viewMode === "field" ? (
        <section className="farm" aria-label="Codex agent farm" onClick={dismissFloating}>
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
      ) : (
        <ClassicDashboard
          workers={classicWorkers}
          connected={connected}
          selectedId={selectedId}
          onSelect={(worker) => setSelectedId(worker.id)}
          onOpen={async (worker) => { await openThread(worker.id, worker.cwd); }}
          onCopy={async (worker) => { await copyResumeCommand(worker.id, worker.threadId, worker.cwd); }}
          onReviewed={removeReviewed}
          onDismiss={dismissFloating}
        />
      )}

      {listOpen && <WorkerList plots={plots} selectedId={selectedId} onSelect={(worker) => { setSelectedId(worker.id); setListOpen(false); }} />}
      {attentionOpen && attentionWorkers.length > 0 && (
        <AttentionList
          workers={attentionWorkers}
          onClose={() => setAttentionOpen(false)}
          onOpen={async (worker) => { await openThread(worker.id, worker.cwd); }}
          onCopy={async (worker) => { await copyResumeCommand(worker.id, worker.threadId, worker.cwd); }}
        />
      )}
      {launcherOpen && settings && (
        <ProjectLauncher
          locations={projectLocations}
          onClose={() => setLauncherOpen(false)}
          onOpenSettings={() => { setLauncherOpen(false); setSettingsOpen(true); }}
        />
      )}
      <Inspector
        worker={selected}
        onClose={() => setSelectedId(null)}
        onOpen={async (worker) => { await openThread(worker.id, worker.cwd); }}
        onReviewed={removeReviewed}
        onCopy={async (worker) => { await copyResumeCommand(worker.id, worker.threadId, worker.cwd); }}
      />
      {settingsOpen && settings && (
        <SettingsPanel
          settings={settings}
          connection={connections}
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
        <UsageLimits hosts={snapshot?.rateLimits ?? []} />
        <span className="farm-footer__hint">{viewMode === "field" ? "Select a worker to inspect their thread" : "Text-only agent monitor"}</span>
        <span>{connectionMessage}</span>
      </footer>
    </main>
  );
}

function FarmPlot({ plot, selectedId, onSelect }: { plot: RepoPlot; selectedId: string | null; onSelect: (worker: Worker) => void }) {
  return (
    <>
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
    </>
  );
}
