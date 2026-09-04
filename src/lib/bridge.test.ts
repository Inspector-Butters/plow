import { describe, expect, it } from "vitest";
import { getAppVersion, resumeCommand, startAgentCommand } from "./bridge";

describe("terminal handoff", () => {
  it("reports the packaged app version", async () => {
    await expect(getAppVersion()).resolves.toBe("0.3.5");
  });

  it("resumes the exact thread in its launch folder", () => {
    expect(resumeCommand("019f5ade-99ad-7ed1-b2f3-159136634cf7", "/Users/demo/My Farm")).toBe(
      "codex resume 019f5ade-99ad-7ed1-b2f3-159136634cf7 --remote unix:// --cd '/Users/demo/My Farm'",
    );
  });

  it("quotes apostrophes in a launch folder", () => {
    expect(resumeCommand("019f5ade-99ad-7ed1-b2f3-159136634cf7", "/tmp/farmer's field")).toContain(
      "'/tmp/farmer'\\''s field'",
    );
  });

  it("starts a shared-daemon session in the selected project", () => {
    expect(startAgentCommand("/Users/demo/My Farm")).toBe(
      "codex --remote unix:// --cd '/Users/demo/My Farm'",
    );
  });
});
