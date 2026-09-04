import { useEffect, useMemo, useState } from "react";
import type { AppUpdateInfo, AppUpdateProgress } from "../lib/updater";

interface UpdatePromptProps {
  update: AppUpdateInfo;
  onDismiss: () => void;
  onInstall: (onProgress: (progress: AppUpdateProgress) => void) => Promise<void>;
}

type InstallState = "ready" | "working" | "error";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function UpdatePrompt({ update, onDismiss, onInstall }: UpdatePromptProps) {
  const [installState, setInstallState] = useState<InstallState>("ready");
  const [progress, setProgress] = useState<AppUpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = installState === "working";

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onDismiss();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, onDismiss]);

  const percent = useMemo(() => {
    if (!progress?.totalBytes) return null;
    return Math.min(100, Math.round((progress.downloadedBytes / progress.totalBytes) * 100));
  }, [progress]);

  const install = async () => {
    setInstallState("working");
    setError(null);
    setProgress({ phase: "downloading", downloadedBytes: 0, totalBytes: null });
    try {
      await onInstall(setProgress);
    } catch (installError) {
      setInstallState("error");
      setError(errorMessage(installError));
    }
  };

  const phaseLabel = progress?.phase === "installing"
    ? "Installing update…"
    : progress?.phase === "restarting"
      ? "Opening the new version…"
      : percent === null
        ? "Downloading update…"
        : `Downloading update… ${percent}%`;

  return (
    <div className="update-backdrop">
      <section className="update-panel" role="dialog" aria-modal="true" aria-labelledby="update-title" aria-describedby="update-description">
        <div className="update-panel__icon" aria-hidden="true">↻</div>
        <p className="update-panel__eyebrow">Plow update</p>
        <h2 id="update-title">Version {update.version} is ready</h2>
        <p id="update-description" className="update-panel__summary">
          You have {update.currentVersion}. Plow can download the signed update, install it, and open the new version for you.
        </p>

        {update.notes && (
          <div className="update-panel__notes">
            <strong>What’s new</strong>
            <p>{update.notes}</p>
          </div>
        )}

        {busy && progress && (
          <div className="update-progress" aria-live="polite">
            <div className="update-progress__row">
              <strong>{phaseLabel}</strong>
              {progress.phase === "downloading" && progress.totalBytes !== null && (
                <span>{formatBytes(progress.downloadedBytes)} / {formatBytes(progress.totalBytes)}</span>
              )}
            </div>
            <div
              className={`update-progress__track${percent === null || progress.phase !== "downloading" ? " update-progress__track--indeterminate" : ""}`}
              role="progressbar"
              aria-label={phaseLabel}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={progress.phase === "downloading" && percent !== null ? percent : undefined}
            >
              <span style={percent !== null && progress.phase === "downloading" ? { width: `${percent}%` } : undefined} />
            </div>
          </div>
        )}

        {error && <p className="update-panel__error" role="alert">Update failed: {error}</p>}

        <div className="update-panel__actions">
          <button type="button" className="button button--quiet" onClick={onDismiss} disabled={busy}>Not now</button>
          <button type="button" className="button button--primary" onClick={() => void install()} disabled={busy} autoFocus>
            {busy ? "Updating…" : installState === "error" ? "Try again" : "Update and restart"}
          </button>
        </div>
      </section>
    </div>
  );
}
