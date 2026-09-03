import type { AttentionItem, RepoPlot, Worker } from "../types";

const attentionStatuses = new Set(["waitingApproval", "waitingInput", "completed", "failed"]);

export function groupWorkers(workers: Worker[]): RepoPlot[] {
  const plots = new Map<string, RepoPlot>();

  for (const worker of workers) {
    const key = worker.repoPath || worker.repoName;
    const plot = plots.get(key) ?? {
      id: key,
      name: worker.repoName,
      path: worker.repoPath,
      workers: [],
    };
    plot.workers.push(worker);
    plots.set(key, plot);
  }

  return [...plots.values()]
    .map((plot) => ({
      ...plot,
      workers: plot.workers.sort((a, b) => {
        if (a.parentId === null && b.parentId !== null) return -1;
        if (a.parentId !== null && b.parentId === null) return 1;
        return b.updatedAt - a.updatedAt;
      }),
    }))
    .sort((a, b) => {
      const aAttention = a.workers.some((worker) => attentionStatuses.has(worker.status));
      const bAttention = b.workers.some((worker) => attentionStatuses.has(worker.status));
      if (aAttention !== bAttention) return aAttention ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
}

export function attentionFor(worker: Worker): AttentionItem | null {
  const key = worker.attentionId ?? `${worker.id}:${worker.status}`;
  switch (worker.status) {
    case "waitingApproval":
      return { key, workerId: worker.id, kind: "approval", title: "Approval needed", body: `${worker.threadName} is waiting for you.` };
    case "waitingInput":
      return { key, workerId: worker.id, kind: "input", title: "A worker has a question", body: `${worker.threadName} needs a little direction.` };
    case "completed":
      return { key, workerId: worker.id, kind: "completed", title: "Work complete", body: `${worker.threadName} is ready for review.` };
    case "failed":
      return { key, workerId: worker.id, kind: "failed", title: "A worker hit a snag", body: `${worker.threadName} needs inspection.` };
    default:
      return null;
  }
}

export function statusLabel(status: Worker["status"]): string {
  return {
    running: "Working",
    waitingApproval: "Approval needed",
    waitingInput: "Question",
    completed: "Ready for review",
    failed: "Needs repair",
  }[status];
}

export function elapsedLabel(worker: Worker, now = Date.now()): string {
  if (!worker.startedAt) return "Started recently";
  const seconds = Math.max(0, Math.floor(now / 1000 - worker.startedAt));
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
}

