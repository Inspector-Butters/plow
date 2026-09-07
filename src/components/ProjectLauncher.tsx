import { useCallback, useEffect, useRef, useState } from "react";
import { listProjects, startAgent } from "../lib/bridge";
import type { ProjectFolder, ProjectLocation } from "../types";

interface ProjectLauncherProps {
  locations: ProjectLocation[];
  onClose: () => void;
  onOpenSettings: () => void;
}

export function ProjectLauncher({ locations, onClose, onOpenSettings }: ProjectLauncherProps) {
  const [hostId, setHostId] = useState(locations[0]?.hostId ?? "");
  const [projects, setProjects] = useState<ProjectFolder[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [startingPath, setStartingPath] = useState<string | null>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const location = locations.find((item) => item.hostId === hostId) ?? locations[0] ?? null;
  const developmentHome = location?.developmentHome ?? "";

  const refresh = useCallback(async () => {
    if (!developmentHome || !location) {
      setProjects([]);
      return;
    }
    setProjects(null);
    setError(null);
    try {
      setProjects(await listProjects(location.hostId));
    } catch (reason) {
      setProjects([]);
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }, [developmentHome, location]);

  useEffect(() => {
    closeRef.current?.focus();
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !startingPath) onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose, startingPath]);

  const launch = async (project: ProjectFolder) => {
    setStartingPath(project.path);
    setError(null);
    try {
      if (!location) throw new Error("Choose a Codex host first");
      await startAgent(location.hostId, project.path);
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setStartingPath(null);
    }
  };

  return (
    <div className="project-backdrop">
      <section className="project-launcher" role="dialog" aria-modal="true" aria-labelledby="project-launcher-title">
        <button ref={closeRef} className="icon-button project-launcher__close" type="button" onClick={onClose} aria-label="Close project picker" disabled={Boolean(startingPath)}>×</button>
        <p className="project-launcher__eyebrow">New Codex session</p>
        <h2 id="project-launcher-title">Choose a project</h2>

        {locations.length > 1 && (
          <div className="settings-field project-launcher__host">
            <label htmlFor="project-host">Run on</label>
            <select id="project-host" value={location?.hostId ?? ""} onChange={(event) => setHostId(event.target.value)} disabled={Boolean(startingPath)}>
              {locations.map((item) => <option key={item.hostId} value={item.hostId}>{item.hostLabel}{item.hostKind === "ssh" ? " — SSH" : ""}</option>)}
            </select>
          </div>
        )}

        {!location ? (
          <div className="project-launcher__setup">
            <strong>No Codex hosts are enabled</strong>
            <span>Enable this computer or add an SSH host in Settings.</span>
          </div>
        ) : !developmentHome ? (
          <div className="project-launcher__setup">
            <strong>Choose a development folder for {location.hostLabel}</strong>
            <span>Plow will list its immediate project folders and launch Codex in the one you select.</span>
          </div>
        ) : (
          <>
            <p className="project-launcher__home" title={developmentHome}>{location.hostKind === "ssh" ? `${location.hostLabel} · ` : ""}{developmentHome}</p>
            <div className="project-launcher__list" aria-label="Start location">
              <button
                className="project-launcher__general"
                type="button"
                onClick={() => void launch({ name: "General", path: developmentHome })}
                disabled={Boolean(startingPath)}
                aria-label="Start general Codex session in development home"
              >
                <span className="project-launcher__home-icon" aria-hidden="true">⌂</span>
                <span><strong>General</strong><small>Development home · {developmentHome}</small></span>
                <span className="project-launcher__arrow" aria-hidden="true">→</span>
              </button>
              {projects === null ? (
                <div className="project-launcher__loading" role="status"><span />Looking for projects…</div>
              ) : projects.length > 0 ? (
                <>
                  <p className="project-launcher__section-label">Project folders</p>
                  {projects.map((project) => (
                    <button
                      key={project.path}
                      type="button"
                      onClick={() => void launch(project)}
                      disabled={Boolean(startingPath)}
                      aria-label={`Start Codex in ${project.name}`}
                    >
                      <span className="project-launcher__folder" aria-hidden="true">▰</span>
                      <span><strong>{project.name}</strong><small>{project.path}</small></span>
                      <span className="project-launcher__arrow" aria-hidden="true">→</span>
                    </button>
                  ))}
                </>
              ) : !error ? (
                <p className="project-launcher__empty">No project folders were found here. You can still start a general session above.</p>
              ) : null}
            </div>
          </>
        )}

        {startingPath && <p className="project-launcher__message" role="status">Opening Codex in the terminal…</p>}
        {error && <p className="project-launcher__error" role="alert">{error}</p>}

        <div className="project-launcher__actions">
          <button className="button button--quiet" type="button" onClick={onOpenSettings} disabled={Boolean(startingPath)}>Settings</button>
          {developmentHome && <button className="button" type="button" onClick={() => void refresh()} disabled={projects === null || Boolean(startingPath)}>Refresh</button>}
        </div>
      </section>
    </div>
  );
}
