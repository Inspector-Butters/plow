import { useEffect, useRef, useState, type FormEvent } from "react";
import { listSshHosts } from "../lib/bridge";
import type { ConnectionInfo, PlowSettings, SshHostSettings } from "../types";

interface SettingsPanelProps {
  settings: PlowSettings;
  connection: ConnectionInfo[];
  onClose: () => void;
  onSave: (settings: PlowSettings) => Promise<void>;
}

export function SettingsPanel({ settings, connection, onClose, onSave }: SettingsPanelProps) {
  const [codexPath, setCodexPath] = useState(settings.codexPath);
  const [developmentHome, setDevelopmentHome] = useState(settings.developmentHome);
  const [localEnabled, setLocalEnabled] = useState(settings.localEnabled);
  const [sshHosts, setSshHosts] = useState(settings.sshHosts);
  const [discoveredHosts, setDiscoveredHosts] = useState<string[]>([]);
  const [notifyWhenUnfocused, setNotifyWhenUnfocused] = useState(settings.notifyWhenUnfocused);
  const [reducedMotion, setReducedMotion] = useState(settings.reducedMotion);
  const [viewMode, setViewMode] = useState(settings.viewMode);
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

  useEffect(() => {
    void listSshHosts().then(setDiscoveredHosts).catch(() => setDiscoveredHosts([]));
  }, []);

  const updateSshHost = (index: number, patch: Partial<SshHostSettings>) => {
    setSshHosts((current) => current.map((host, hostIndex) => hostIndex === index ? { ...host, ...patch } : host));
  };

  const addSshHost = () => {
    const used = new Set(sshHosts.map((host) => host.alias));
    const alias = discoveredHosts.find((host) => !used.has(host)) ?? "";
    setSshHosts((current) => [...current, { alias, codexPath: "", developmentHome: "", enabled: true }]);
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await onSave({
        ...settings,
        localEnabled,
        codexPath: codexPath.trim(),
        developmentHome: developmentHome.trim(),
        sshHosts: sshHosts.map((host) => ({
          ...host,
          alias: host.alias.trim(),
          codexPath: host.codexPath.trim(),
          developmentHome: host.developmentHome.trim(),
        })),
        notifyWhenUnfocused,
        reducedMotion,
        viewMode,
      });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setSaving(false);
    }
  };

  return (
    <div className="settings-backdrop" onMouseDown={(event) => {
      if (event.target === event.currentTarget && !saving) onClose();
    }}>
      <form className="settings-panel" role="dialog" aria-modal="true" aria-labelledby="settings-title" onSubmit={(event) => void submit(event)}>
        <button className="icon-button settings-panel__close" type="button" onClick={onClose} aria-label="Close settings">×</button>
        <p className="settings-panel__eyebrow">Connections</p>
        <h2 id="settings-title">Plow settings</h2>

        <section className="settings-connection-group" aria-labelledby="local-connection-title">
          <div className="settings-connection-group__heading">
            <div><strong id="local-connection-title">This computer</strong><small>Local Codex daemon</small></div>
            <label className="settings-toggle"><input ref={inputRef} type="checkbox" checked={localEnabled} onChange={(event) => setLocalEnabled(event.target.checked)} /><span>{localEnabled ? "Enabled" : "Disabled"}</span></label>
          </div>

          <div className="settings-field">
            <label htmlFor="codex-path">Codex executable</label>
            <input
              id="codex-path"
              type="text"
              value={codexPath}
              onChange={(event) => setCodexPath(event.target.value)}
              placeholder="Automatic detection"
              aria-describedby="codex-path-help"
              autoComplete="off"
              spellCheck={false}
              disabled={!localEnabled}
            />
            <small id="codex-path-help">Leave blank for automatic detection, or enter the absolute path to a daemon-capable Codex executable.</small>
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
              disabled={!localEnabled}
            />
            <small id="development-home-help">The immediate project folders shown by Start agent.</small>
          </div>
        </section>

        <section className="settings-ssh" aria-labelledby="ssh-connections-title">
          <div className="settings-ssh__heading">
            <div><strong id="ssh-connections-title">SSH hosts</strong><small>Uses your existing OpenSSH configuration and keys</small></div>
            <button className="button button--quiet" type="button" onClick={addSshHost}>Add host</button>
          </div>
          <datalist id="ssh-host-aliases">
            {discoveredHosts.map((host) => <option value={host} key={host} />)}
          </datalist>
          {sshHosts.length === 0 ? (
            <p className="settings-ssh__empty">Add a host alias from <code>~/.ssh/config</code> to monitor Codex on another machine.</p>
          ) : sshHosts.map((host, index) => (
            <fieldset className="settings-ssh-host" key={`${host.alias}:${index}`}>
              <legend>SSH connection {index + 1}</legend>
              <div className="settings-ssh-host__toolbar">
                <label className="settings-toggle"><input type="checkbox" checked={host.enabled} onChange={(event) => updateSshHost(index, { enabled: event.target.checked })} /><span>{host.enabled ? "Enabled" : "Disabled"}</span></label>
                <button className="button button--quiet" type="button" onClick={() => setSshHosts((current) => current.filter((_, hostIndex) => hostIndex !== index))}>Remove</button>
              </div>
              <div className="settings-field">
                <label htmlFor={`ssh-alias-${index}`}>SSH host alias {index + 1}</label>
                <input id={`ssh-alias-${index}`} list="ssh-host-aliases" value={host.alias} onChange={(event) => updateSshHost(index, { alias: event.target.value })} placeholder="devbox" autoComplete="off" spellCheck={false} />
                <small>Use a concrete Host alias that already works with <code>ssh {host.alias || "devbox"}</code>.</small>
              </div>
              <div className="settings-field">
                <label htmlFor={`ssh-codex-${index}`}>Remote Codex command {index + 1}</label>
                <input id={`ssh-codex-${index}`} value={host.codexPath} onChange={(event) => updateSshHost(index, { codexPath: event.target.value })} placeholder="codex" autoComplete="off" spellCheck={false} />
                <small>Leave blank when Codex is on the remote login shell PATH, or enter its absolute remote path.</small>
              </div>
              <div className="settings-field">
                <label htmlFor={`ssh-home-${index}`}>Remote development home {index + 1}</label>
                <input id={`ssh-home-${index}`} value={host.developmentHome} onChange={(event) => updateSshHost(index, { developmentHome: event.target.value })} placeholder="/home/you/Developer" autoComplete="off" spellCheck={false} />
                <small>Optional for monitoring; required to start agents from Plow.</small>
              </div>
            </fieldset>
          ))}
        </section>

        <div className="settings-panel__note">
          Plow never stores SSH keys or passwords. Remote hosts use your SSH config and must have the standalone Codex installation authenticated for that remote user.
        </div>

        <div className="settings-field">
          <label htmlFor="view-mode">Agent view</label>
          <select id="view-mode" value={viewMode} onChange={(event) => setViewMode(event.target.value as PlowSettings["viewMode"])}>
            <option value="field">Field — animated farm</option>
            <option value="classic">Classic — text and controls</option>
          </select>
          <small>Classic view removes the farm artwork and worker animation while keeping every agent action available.</small>
        </div>

        {connection && connection.length > 0 && (
          <div className="settings-panel__connections" aria-label="Connection status">
            {connection.map((item) => (
              <div className={`settings-panel__connection settings-panel__connection--${item.status}`} key={item.hostId}>
                <strong>{item.hostLabel}: {item.status}</strong>
                <span>{item.message}</span>
                {item.codexPath && <code title={item.codexPath}>Using {item.codexPath}</code>}
              </div>
            ))}
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
