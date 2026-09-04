import { useState } from "react";
import type { Worker } from "../types";
import { elapsedLabel, statusLabel } from "../lib/workers";

interface ClassicDashboardProps {
  workers: Worker[];
  connected: boolean;
  selectedId: string | null;
  onSelect: (worker: Worker) => void;
  onOpen: (worker: Worker) => Promise<void>;
  onCopy: (worker: Worker) => Promise<void>;
  onReviewed: (worker: Worker) => Promise<void>;
}

export function ClassicDashboard({ workers, connected, selectedId, onSelect, onOpen, onCopy, onReviewed }: ClassicDashboardProps) {
  const [messages, setMessages] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);

  const run = async (worker: Worker, actionName: string, action: () => Promise<void>, success: string) => {
    const key = `${worker.id}:${actionName}`;
    setBusy(key);
    setMessages((current) => ({ ...current, [worker.id]: "" }));
    try {
      await action();
      setMessages((current) => ({ ...current, [worker.id]: success }));
    } catch (error) {
      setMessages((current) => ({
        ...current,
        [worker.id]: error instanceof Error ? error.message : "That action did not work.",
      }));
    } finally {
      setBusy((current) => current === key ? null : current);
    }
  };

  return (
    <section className="classic-dashboard" aria-label="Codex agents">
      <header className="classic-dashboard__header">
        <div>
          <p>Classic view</p>
          <h2>Codex agents</h2>
        </div>
        <strong>{workers.length} active</strong>
      </header>

      {workers.length === 0 ? (
        <div className="classic-dashboard__empty">
          <h3>{connected ? "No active agents" : "Connecting to Codex"}</h3>
          <p>{connected ? "Start a Codex session and it will appear here." : "Plow is looking for the local Codex daemon."}</p>
        </div>
      ) : (
        <div className="classic-agents">
          <div className="classic-agent classic-agent--head" aria-hidden="true">
            <span>Agent</span><span>Thread</span><span>Status</span><span>Runtime</span><span>Actions</span>
          </div>
          {workers.map((worker) => {
            const canReview = worker.attentionId && (worker.status === "completed" || worker.status === "failed");
            const openKey = `${worker.id}:open`;
            const copyKey = `${worker.id}:copy`;
            const reviewKey = `${worker.id}:review`;
            return (
              <article className={`classic-agent${selectedId === worker.id ? " classic-agent--selected" : ""}`} key={worker.id}>
                <div className="classic-agent__identity">
                  <strong>{worker.displayName}</strong>
                  <small>{worker.parentId ? "Crew member" : "Lead agent"}</small>
                </div>
                <div className="classic-agent__thread">
                  <strong>{worker.threadName}</strong>
                  <small title={worker.cwd}>{worker.branch ?? worker.cwd}</small>
                </div>
                <div className={`classic-agent__status classic-agent__status--${worker.status}`}>
                  <strong>{statusLabel(worker.status)}</strong>
                  <small>{worker.activity}</small>
                </div>
                <div className="classic-agent__runtime">
                  <strong>{elapsedLabel(worker)}</strong>
                  <small>{worker.model ?? worker.source}</small>
                </div>
                <div className="classic-agent__actions">
                  <button className="button button--primary" type="button" disabled={busy === openKey} onClick={() => void run(worker, "open", () => onOpen(worker), "Opening Codex…")}>Open terminal</button>
                  <button className="button button--quiet" type="button" disabled={busy === copyKey} onClick={() => void run(worker, "copy", () => onCopy(worker), "Resume command copied")}>Copy command</button>
                  {canReview && <button className="button" type="button" disabled={busy === reviewKey} onClick={() => void run(worker, "review", () => onReviewed(worker), "Marked reviewed")}>Mark reviewed</button>}
                  <button className="button button--quiet" type="button" onClick={() => onSelect(worker)}>Details</button>
                </div>
                {messages[worker.id] && <p className="classic-agent__message" role="status">{messages[worker.id]}</p>}
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}
