import { describe, expect, it } from "vitest";
import { attentionFor, groupWorkers, statusLabel } from "./workers";
import { demoWorkers } from "./mock";

describe("worker presentation", () => {
  it("groups workers by repository and keeps lead workers first", () => {
    const plots = groupWorkers([...demoWorkers]);
    const plow = plots.find((plot) => plot.name === "plow");
    expect(plow?.workers).toHaveLength(2);
    expect(plow?.workers[0].parentId).toBeNull();
  });

  it("puts plots needing attention before ordinary work", () => {
    const quietWorker = {
      ...demoWorkers[0],
      id: "quiet-worker",
      repoName: "quiet-repo",
      repoPath: "/tmp/quiet-repo",
    };
    const plots = groupWorkers([...demoWorkers, quietWorker]);
    expect(plots.at(-1)?.name).toBe("quiet-repo");
  });

  it("creates stable attention copy", () => {
    const approval = demoWorkers.find((worker) => worker.status === "waitingApproval");
    expect(approval && attentionFor(approval)).toMatchObject({ kind: "approval", key: "demo-approval" });
    expect(statusLabel("completed")).toBe("Ready for review");
  });
});
