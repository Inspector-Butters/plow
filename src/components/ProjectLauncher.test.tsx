import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { listProjects, startAgent } from "../lib/bridge";
import { ProjectLauncher } from "./ProjectLauncher";

vi.mock("../lib/bridge", () => ({
  listProjects: vi.fn(),
  startAgent: vi.fn(),
}));

const localLocation = {
  hostId: "local",
  hostLabel: "This computer",
  hostKind: "local" as const,
  developmentHome: "/home/farmer/Developer",
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("ProjectLauncher", () => {
  it("closes when its backdrop is clicked", () => {
    vi.mocked(listProjects).mockResolvedValue([]);
    const onClose = vi.fn();
    const { container } = render(
      <ProjectLauncher locations={[localLocation]} onClose={onClose} onOpenSettings={() => undefined} />,
    );

    fireEvent.mouseDown(container.querySelector(".project-backdrop")!);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("starts a general session in the development home", async () => {
    vi.mocked(listProjects).mockResolvedValue([{ name: "plow", path: "/home/farmer/Developer/plow" }]);
    vi.mocked(startAgent).mockResolvedValue("Opening Codex");
    const onClose = vi.fn();
    const { getByRole } = render(
      <ProjectLauncher locations={[localLocation]} onClose={onClose} onOpenSettings={() => undefined} />,
    );

    fireEvent.click(getByRole("button", { name: "Start general Codex session in development home" }));

    await waitFor(() => expect(startAgent).toHaveBeenCalledWith("local", "/home/farmer/Developer"));
    expect(onClose).toHaveBeenCalled();
  });

  it("lists development folders and starts the selected project", async () => {
    vi.mocked(listProjects).mockResolvedValue([{ name: "plow", path: "/home/farmer/Developer/plow" }]);
    vi.mocked(startAgent).mockResolvedValue("Opening Codex");
    const onClose = vi.fn();
    const { findByRole } = render(
      <ProjectLauncher locations={[localLocation]} onClose={onClose} onOpenSettings={() => undefined} />,
    );

    fireEvent.click(await findByRole("button", { name: "Start Codex in plow" }));

    await waitFor(() => expect(startAgent).toHaveBeenCalledWith("local", "/home/farmer/Developer/plow"));
    expect(onClose).toHaveBeenCalled();
  });

  it("directs the user to settings when no development home is configured", () => {
    const onOpenSettings = vi.fn();
    const { getByRole, getByText } = render(
      <ProjectLauncher locations={[{ ...localLocation, developmentHome: "" }]} onClose={() => undefined} onOpenSettings={onOpenSettings} />,
    );

    expect(getByText("Choose a development folder for This computer")).toBeInTheDocument();
    fireEvent.click(getByRole("button", { name: "Settings" }));
    expect(onOpenSettings).toHaveBeenCalled();
    expect(listProjects).not.toHaveBeenCalled();
  });

  it("switches to an SSH host before listing and launching its projects", async () => {
    vi.mocked(listProjects).mockImplementation(async (hostId) => hostId === "ssh:devbox"
      ? [{ name: "remote-api", path: "/srv/dev/remote-api" }]
      : []);
    vi.mocked(startAgent).mockResolvedValue("Opening remote Codex");
    const { findByRole, getByLabelText } = render(
      <ProjectLauncher
        locations={[localLocation, { hostId: "ssh:devbox", hostLabel: "devbox", hostKind: "ssh", developmentHome: "/srv/dev" }]}
        onClose={() => undefined}
        onOpenSettings={() => undefined}
      />,
    );

    fireEvent.change(getByLabelText("Run on"), { target: { value: "ssh:devbox" } });
    fireEvent.click(await findByRole("button", { name: "Start Codex in remote-api" }));
    await waitFor(() => expect(startAgent).toHaveBeenCalledWith("ssh:devbox", "/srv/dev/remote-api"));
  });
});
