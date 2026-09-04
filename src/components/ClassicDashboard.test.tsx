import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { demoWorkers } from "../lib/mock";
import { ClassicDashboard } from "./ClassicDashboard";

afterEach(cleanup);

describe("ClassicDashboard", () => {
  it("shows agents as text with operational controls and no artwork", async () => {
    const onOpen = vi.fn().mockResolvedValue(undefined);
    const onCopy = vi.fn().mockResolvedValue(undefined);
    const onSelect = vi.fn();
    const { container, getAllByRole, getByRole, getByText } = render(
      <ClassicDashboard
        workers={[demoWorkers[0]]}
        connected
        selectedId={null}
        onSelect={onSelect}
        onOpen={onOpen}
        onCopy={onCopy}
        onReviewed={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(getByText(demoWorkers[0].displayName)).toBeInTheDocument();
    expect(getByText(demoWorkers[0].threadName)).toBeInTheDocument();
    expect(container.querySelector("img")).not.toBeInTheDocument();
    fireEvent.click(getByRole("button", { name: "Open terminal" }));
    fireEvent.click(getByRole("button", { name: "Copy command" }));
    fireEvent.click(getAllByRole("button", { name: "Details" })[0]);

    await waitFor(() => expect(onOpen).toHaveBeenCalledWith(demoWorkers[0]));
    expect(onCopy).toHaveBeenCalledWith(demoWorkers[0]);
    expect(onSelect).toHaveBeenCalledWith(demoWorkers[0]);
  });

  it("lets completed agents be marked reviewed", async () => {
    const onReviewed = vi.fn().mockResolvedValue(undefined);
    const worker = demoWorkers.find((item) => item.status === "completed")!;
    const { getByRole } = render(
      <ClassicDashboard
        workers={[worker]}
        connected
        selectedId={null}
        onSelect={() => undefined}
        onOpen={vi.fn().mockResolvedValue(undefined)}
        onCopy={vi.fn().mockResolvedValue(undefined)}
        onReviewed={onReviewed}
      />,
    );

    fireEvent.click(getByRole("button", { name: "Mark reviewed" }));
    await waitFor(() => expect(onReviewed).toHaveBeenCalledWith(worker));
  });
});
