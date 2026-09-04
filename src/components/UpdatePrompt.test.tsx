import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppUpdateInfo, AppUpdateProgress } from "../lib/updater";
import { UpdatePrompt } from "./UpdatePrompt";

const update: AppUpdateInfo = {
  currentVersion: "0.2.1",
  version: "0.3.0",
  date: "2026-09-04T12:00:00Z",
  notes: "A smoother, safer update flow.",
};

afterEach(cleanup);

describe("UpdatePrompt", () => {
  it("explains the available version and can defer it", () => {
    const onDismiss = vi.fn();
    const { getByRole, getByText } = render(
      <UpdatePrompt update={update} onDismiss={onDismiss} onInstall={vi.fn()} />,
    );

    expect(getByRole("dialog")).toHaveAccessibleName("Version 0.3.0 is ready");
    expect(getByText(/You have 0.2.1/)).toBeInTheDocument();
    expect(getByText("A smoother, safer update flow.")).toBeInTheDocument();
    fireEvent.click(getByRole("button", { name: "Not now" }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("shows download progress after approval", async () => {
    let reportProgress: ((progress: AppUpdateProgress) => void) | undefined;
    const onInstall = vi.fn((report: (progress: AppUpdateProgress) => void) => {
      reportProgress = report;
      return new Promise<void>(() => undefined);
    });
    const { getByRole } = render(
      <UpdatePrompt update={update} onDismiss={() => undefined} onInstall={onInstall} />,
    );

    fireEvent.click(getByRole("button", { name: "Update and restart" }));
    await waitFor(() => expect(onInstall).toHaveBeenCalledOnce());
    act(() => reportProgress?.({ phase: "downloading", downloadedBytes: 50, totalBytes: 100 }));

    expect(getByRole("progressbar", { name: "Downloading update… 50%" })).toHaveAttribute("aria-valuenow", "50");
    expect(getByRole("button", { name: "Not now" })).toBeDisabled();
  });

  it("keeps the prompt open and offers a retry after an install error", async () => {
    const onInstall = vi.fn().mockRejectedValue(new Error("signature could not be verified"));
    const { findByRole, getByRole } = render(
      <UpdatePrompt update={update} onDismiss={() => undefined} onInstall={onInstall} />,
    );

    fireEvent.click(getByRole("button", { name: "Update and restart" }));

    expect(await findByRole("alert")).toHaveTextContent("signature could not be verified");
    expect(getByRole("button", { name: "Try again" })).toBeEnabled();
    expect(getByRole("dialog")).toBeInTheDocument();
  });
});
