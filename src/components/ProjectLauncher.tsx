import { useCallback, useEffect, useRef, useState } from "react";
import { listProjects, startAgent } from "../lib/bridge";
import type { ProjectFolder } from "../types";

interface ProjectLauncherProps {
  developmentHome: string;
  onClose: () => void;
  onOpenSettings: () => void;
}

export function ProjectLauncher({ developmentHome, onClose, onOpenSettings }: ProjectLauncherProps) {
  const [projects, setProjects] = useState<ProjectFolder[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [startingPath, setStartingPath] = useState<string | null>(null);
  const closeRef = useRef<HTMLButtonElement>(null);

  const refresh = useCallback(async () => {
    if (!developmentHome) {
      setProjects([]);
      return;
    }
    setProjects(null);
    setError(null);
    try {
      setProjects(await listProjects());
    } catch (reason) {
      setProjects([]);
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }, [developmentHome]);

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
      await startAgent(project.path);
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

        {!developmentHome ? (
          <div className="project-launcher__setup">
            <strong>Choose your development folder first</strong>
            <span>Plow will list each project folder here and launch Codex in the one you select.</span>
          </div>
        ) : (
          <>
            <p className="project-launcher__home" title={developmentHome}>{developmentHome}</p>
            {projects === null ? (
              <div className="project-launcher__loading" role="status"><span />Looking for projects…</div>
            ) : projects.length > 0 ? (
              <div className="project-launcher__list" aria-label="Project folders">
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
              </div>
            ) : !error ? (
              <p className="project-launcher__empty">No project folders were found here.</p>
            ) : null}
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
