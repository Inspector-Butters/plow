import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  check: vi.fn(),
  relaunch: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("./bridge", () => ({ isNativeApp: () => true }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: mocks.check }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }));

describe("app updater", () => {
  it("checks, reports download progress, installs, and relaunches", async () => {
    mocks.check.mockResolvedValue({
      currentVersion: "0.3.0",
      version: "0.3.1",
      date: "2026-09-05T10:00:00Z",
      body: "A small farm improvement.",
      downloadAndInstall: async (onEvent: (event: unknown) => void) => {
        onEvent({ event: "Started", data: { contentLength: 100 } });
        onEvent({ event: "Progress", data: { chunkLength: 40 } });
        onEvent({ event: "Finished" });
      },
      close: vi.fn(),
    });
    const { checkForAppUpdate, installAppUpdate } = await import("./updater");
    const progress = vi.fn();

    await expect(checkForAppUpdate()).resolves.toEqual({
      currentVersion: "0.3.0",
      version: "0.3.1",
      date: "2026-09-05T10:00:00Z",
      notes: "A small farm improvement.",
    });
    await installAppUpdate(progress);

    expect(mocks.check).toHaveBeenCalledWith({ timeout: 15_000 });
    expect(progress).toHaveBeenNthCalledWith(2, {
      phase: "downloading",
      downloadedBytes: 40,
      totalBytes: 100,
    });
    expect(progress).toHaveBeenLastCalledWith({
      phase: "restarting",
      downloadedBytes: 40,
      totalBytes: 100,
    });
    expect(mocks.relaunch).toHaveBeenCalledOnce();
  });
});
