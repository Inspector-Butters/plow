import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlowSettings } from "../types";
import { SettingsPanel } from "./SettingsPanel";

const settings: PlowSettings = {
  notifyWhenUnfocused: true,
  keepInTray: true,
  reducedMotion: false,
  codexPath: "",
  developmentHome: "",
  viewMode: "field",
};

afterEach(cleanup);

describe("SettingsPanel", () => {
  it("saves a trimmed Codex executable path", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { getByLabelText, getByRole } = render(
      <SettingsPanel settings={settings} connection={null} onClose={() => undefined} onSave={onSave} />,
    );

    fireEvent.change(getByLabelText("Codex executable"), {
      target: { value: "  /opt/codex/bin/codex  " },
    });
    fireEvent.change(getByLabelText("Development home folder"), {
      target: { value: "  /home/farmer/Developer  " },
    });
    fireEvent.click(getByRole("button", { name: "Save settings" }));

    await waitFor(() => expect(onSave).toHaveBeenCalledWith({
      ...settings,
      codexPath: "/opt/codex/bin/codex",
      developmentHome: "/home/farmer/Developer",
    }));
  });

  it("saves the classic agent view", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { getByLabelText, getByRole } = render(
      <SettingsPanel settings={settings} connection={null} onClose={() => undefined} onSave={onSave} />,
    );

    fireEvent.change(getByLabelText("Agent view"), { target: { value: "classic" } });
    fireEvent.click(getByRole("button", { name: "Save settings" }));

    await waitFor(() => expect(onSave).toHaveBeenCalledWith({ ...settings, viewMode: "classic" }));
  });

  it("keeps the dialog open and reports path validation errors", async () => {
    const onSave = vi.fn().mockRejectedValue(new Error("No executable Codex file was found"));
    const { findByRole, getByRole } = render(
      <SettingsPanel settings={settings} connection={null} onClose={() => undefined} onSave={onSave} />,
    );

    fireEvent.click(getByRole("button", { name: "Save settings" }));

    expect(await findByRole("alert")).toHaveTextContent("No executable Codex file was found");
    expect(getByRole("dialog")).toBeInTheDocument();
  });
});
