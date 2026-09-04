import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { listProjects, startAgent } from "../lib/bridge";
import { ProjectLauncher } from "./ProjectLauncher";

vi.mock("../lib/bridge", () => ({
  listProjects: vi.fn(),
  startAgent: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("ProjectLauncher", () => {
  it("lists development folders and starts the selected project", async () => {
    vi.mocked(listProjects).mockResolvedValue([{ name: "plow", path: "/home/farmer/Developer/plow" }]);
    vi.mocked(startAgent).mockResolvedValue("Opening Codex");
    const onClose = vi.fn();
    const { findByRole } = render(
      <ProjectLauncher developmentHome="/home/farmer/Developer" onClose={onClose} onOpenSettings={() => undefined} />,
    );

    fireEvent.click(await findByRole("button", { name: "Start Codex in plow" }));

    await waitFor(() => expect(startAgent).toHaveBeenCalledWith("/home/farmer/Developer/plow"));
    expect(onClose).toHaveBeenCalled();
  });

  it("directs the user to settings when no development home is configured", () => {
    const onOpenSettings = vi.fn();
    const { getByRole, getByText } = render(
      <ProjectLauncher developmentHome="" onClose={() => undefined} onOpenSettings={onOpenSettings} />,
    );

    expect(getByText("Choose your development folder first")).toBeInTheDocument();
    fireEvent.click(getByRole("button", { name: "Settings" }));
    expect(onOpenSettings).toHaveBeenCalled();
    expect(listProjects).not.toHaveBeenCalled();
  });
});
