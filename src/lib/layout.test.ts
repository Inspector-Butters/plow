import { describe, expect, it } from "vitest";
import { motionDelay, plotPosition, workerPosition } from "./layout";

describe("farm layout", () => {
  it("gives a worker the same place independently of list membership or order", () => {
    const original = workerPosition("/projects/plow", "worker-a");
    workerPosition("/projects/plow", "new-worker");
    expect(workerPosition("/projects/plow", "worker-a")).toEqual(original);
  });

  it("keeps plots and workers inside the usable field", () => {
    const plot = plotPosition("/projects/a-repository");
    const worker = workerPosition("/projects/a-repository", "worker-99");
    expect(plot.x).toBeGreaterThanOrEqual(0);
    expect(plot.x).toBeLessThanOrEqual(100);
    expect(worker.x).toBeGreaterThanOrEqual(5);
    expect(worker.x).toBeLessThanOrEqual(95);
    expect(worker.y).toBeGreaterThanOrEqual(25);
    expect(worker.y).toBeLessThanOrEqual(79);
  });

  it("assigns a stable negative animation phase", () => {
    expect(motionDelay("worker-a")).toBe(motionDelay("worker-a"));
    expect(motionDelay("worker-a")).toBeLessThanOrEqual(0);
  });
});
