import type { RepoPlot, Worker } from "../types";
import { statusLabel } from "../lib/workers";

interface WorkerListProps {
  plots: RepoPlot[];
  selectedId: string | null;
  onSelect: (worker: Worker) => void;
}

export function WorkerList({ plots, selectedId, onSelect }: WorkerListProps) {
  return (
    <section className="worker-list" aria-label="Active Codex workers">
      {plots.map((plot) => (
        <div className="worker-list__plot" key={plot.id}>
          <h3>{plot.name}</h3>
          {plot.workers.map((worker) => (
            <button
              type="button"
              key={worker.id}
              className={selectedId === worker.id ? "is-selected" : ""}
              onClick={() => onSelect(worker)}
            >
              <span className={`status-dot status-dot--${worker.status}`} />
              <span><strong>{worker.threadName}</strong><small>{statusLabel(worker.status)}</small></span>
            </button>
          ))}
        </div>
      ))}
    </section>
  );
}

