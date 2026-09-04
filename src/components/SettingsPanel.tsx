import { useEffect, useRef, useState, type FormEvent } from "react";
import type { ConnectionInfo, PlowSettings } from "../types";

interface SettingsPanelProps {
  settings: PlowSettings;
  connection: ConnectionInfo | null;
  onClose: () => void;
  onSave: (settings: PlowSettings) => Promise<void>;
}

export function SettingsPanel({ settings, connection, onClose, onSave }: SettingsPanelProps) {
  const [codexPath, setCodexPath] = useState(settings.codexPath);
  const [developmentHome, setDevelopmentHome] = useState(settings.developmentHome);
  const [notifyWhenUnfocused, setNotifyWhenUnfocused] = useState(settings.notifyWhenUnfocused);
  const [reducedMotion, setReducedMotion] = useState(settings.reducedMotion);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await onSave({
        ...settings,
        codexPath: codexPath.trim(),
        developmentHome: developmentHome.trim(),
        notifyWhenUnfocused,
        reducedMotion,
      });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setSaving(false);
    }
  };

  return (
    <div className="settings-backdrop">
      <form className="settings-panel" role="dialog" aria-modal="true" aria-labelledby="settings-title" onSubmit={(event) => void submit(event)}>
        <button className="icon-button settings-panel__close" type="button" onClick={onClose} aria-label="Close settings">×</button>
        <p className="settings-panel__eyebrow">Local connection</p>
        <h2 id="settings-title">Plow settings</h2>

        <div className="settings-field">
          <label htmlFor="codex-path">Codex executable</label>
          <input
            ref={inputRef}
            id="codex-path"
            type="text"
            value={codexPath}
            onChange={(event) => setCodexPath(event.target.value)}
            placeholder="Automatic detection"
            aria-describedby="codex-path-help"
            autoComplete="off"
            spellCheck={false}
          />
          <small id="codex-path-help">Leave blank to prefer the managed standalone install automatically, or enter an absolute path to a Codex executable.</small>
        </div>

        <div className="settings-panel__note">
          The shared daemon requires the standalone Codex installation. An npm or Homebrew executable can run Codex normally but may not be able to start this daemon.
        </div>

        <div className="settings-field">
          <label htmlFor="development-home">Development home folder</label>
          <input
            id="development-home"
            type="text"
            value={developmentHome}
            onChange={(event) => setDevelopmentHome(event.target.value)}
            placeholder="/home/you/Developer"
            aria-describedby="development-home-help"
            autoComplete="off"
            spellCheck={false}
          />
          <small id="development-home-help">Plow lists the immediate project folders here when you choose Start agent. Enter an absolute folder path.</small>
        </div>

        {connection && (
          <div className={`settings-panel__connection settings-panel__connection--${connection.status}`}>
            <strong>Connection: {connection.status}</strong>
            <span>{connection.message}</span>
            {connection.codexPath && <code title={connection.codexPath}>Using {connection.codexPath}</code>}
          </div>
        )}

        <label className="settings-check">
          <input type="checkbox" checked={notifyWhenUnfocused} onChange={(event) => setNotifyWhenUnfocused(event.target.checked)} />
          <span>Notify when Plow is unfocused</span>
        </label>
        <label className="settings-check">
          <input type="checkbox" checked={reducedMotion} onChange={(event) => setReducedMotion(event.target.checked)} />
          <span>Reduce motion</span>
        </label>

        {error && <p className="settings-panel__error" role="alert">{error}</p>}
        <div className="settings-panel__actions">
          <button className="button button--quiet" type="button" onClick={onClose}>Cancel</button>
          <button className="button button--primary" type="submit" disabled={saving}>{saving ? "Saving…" : "Save settings"}</button>
        </div>
      </form>
    </div>
  );
}
