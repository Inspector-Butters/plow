import { useState } from "react";
import type { Worker } from "../types";
import { statusLabel } from "../lib/workers";

interface AttentionListProps {
  workers: Worker[];
  onClose: () => void;
  onOpen: (worker: Worker) => Promise<void>;
  onCopy: (worker: Worker) => Promise<void>;
}

export function AttentionList({ workers, onClose, onOpen, onCopy }: AttentionListProps) {
  const [busy, setBusy] = useState<string | null>(null);
  const [messages, setMessages] = useState<Record<string, string>>({});

  const run = async (worker: Worker, action: "open" | "copy") => {
    const key = `${worker.id}:${action}`;
    setBusy(key);
    setMessages((current) => ({ ...current, [worker.id]: "" }));
    try {
      if (action === "open") await onOpen(worker);
      else await onCopy(worker);
      setMessages((current) => ({
        ...current,
        [worker.id]: action === "open" ? "Opening Codex…" : "Resume command copied",
      }));
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
    <section className="attention-list" aria-label="Agents needing attention">
      <header>
        <div><strong>Needs attention</strong><small>{workers.length} agent{workers.length === 1 ? "" : "s"}</small></div>
        <button className="icon-button attention-list__close" type="button" onClick={onClose} aria-label="Close attention list">×</button>
      </header>
      <div className="attention-list__items">
        {workers.map((worker) => (
          <article className={`attention-list__item attention-list__item--${worker.status}`} key={worker.id}>
            <div className="attention-list__identity">
              <span className={`status-dot status-dot--${worker.status}`} />
              <span>
                <strong>{worker.displayName}</strong>
                <small>{statusLabel(worker.status)} · {worker.hostLabel}</small>
              </span>
            </div>
            <p title={worker.threadName}>{worker.threadName}</p>
            <div className="attention-list__actions">
              <button className="button button--primary" type="button" disabled={busy === `${worker.id}:open`} onClick={() => void run(worker, "open")}>Open terminal</button>
              <button className="button button--quiet" type="button" disabled={busy === `${worker.id}:copy`} onClick={() => void run(worker, "copy")}>Copy command</button>
            </div>
            {messages[worker.id] && <small className="attention-list__message" role="status">{messages[worker.id]}</small>}
          </article>
        ))}
      </div>
    </section>
  );
}
