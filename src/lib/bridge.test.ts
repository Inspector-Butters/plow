import { describe, expect, it } from "vitest";
import { resumeCommand } from "./bridge";

describe("terminal handoff", () => {
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
});
