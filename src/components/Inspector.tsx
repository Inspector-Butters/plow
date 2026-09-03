import { useEffect, useState } from "react";
import type { Worker } from "../types";
import { elapsedLabel, statusLabel } from "../lib/workers";

interface InspectorProps {
  worker: Worker | null;
  onClose: () => void;
  onOpen: (worker: Worker) => Promise<void>;
  onReviewed: (worker: Worker) => Promise<void>;
  onCopy: (worker: Worker) => Promise<void>;
}

export function Inspector({ worker, onClose, onOpen, onReviewed, onCopy }: InspectorProps) {
  const [message, setMessage] = useState("");

  useEffect(() => setMessage(""), [worker?.id]);

  if (!worker) return null;
  const canReview = worker.attentionId && (worker.status === "completed" || worker.status === "failed");

  const run = async (action: () => Promise<void>, success: string) => {
    setMessage("");
    try {
      await action();
      setMessage(success);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "That did not work.");
    }
  };

  return (
    <aside className="inspector" aria-label="Selected worker details">
      <button className="icon-button inspector__close" type="button" onClick={onClose} aria-label="Close worker details">×</button>
      <div className="inspector__eyebrow">{worker.parentId ? "Crew member" : "Lead worker"}</div>
      <h2>{worker.threadName}</h2>
      <div className={`inspector__status inspector__status--${worker.status}`}>
        <span className={`status-dot status-dot--${worker.status}`} />
        {statusLabel(worker.status)}
      </div>

      <dl className="inspector__facts">
        <div><dt>Field</dt><dd>{worker.repoName}</dd></div>
        {worker.branch && <div><dt>Branch</dt><dd>{worker.branch}</dd></div>}
        <div><dt>Job</dt><dd>{worker.activity}</dd></div>
        <div><dt>Elapsed</dt><dd>{elapsedLabel(worker)}</dd></div>
        {worker.model && <div><dt>Model</dt><dd>{worker.model}</dd></div>}
        <div><dt>Source</dt><dd>{worker.source}</dd></div>
      </dl>

      <div className="inspector__actions">
        <button className="button button--primary" type="button" onClick={() => void run(() => onOpen(worker), "Opening Codex…")}>Open in terminal</button>
        {canReview && <button className="button" type="button" onClick={() => void run(() => onReviewed(worker), "Marked reviewed")}>Mark reviewed</button>}
        <button className="button button--quiet" type="button" onClick={() => void run(() => onCopy(worker), "Resume command copied")}>Copy command</button>
      </div>
      {message && <p className="inspector__message" role="status">{message}</p>}
      <p className="inspector__path" title={worker.cwd}>{worker.cwd}</p>
    </aside>
  );
}

