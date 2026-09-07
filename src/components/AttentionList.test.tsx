import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { demoWorkers } from "../lib/mock";
import { AttentionList } from "./AttentionList";

afterEach(cleanup);

describe("AttentionList", () => {
  it("lists attention workers and provides terminal and copy actions", async () => {
    const workers = demoWorkers.filter((worker) => worker.status !== "running").slice(0, 2);
    const onOpen = vi.fn().mockResolvedValue(undefined);
    const onCopy = vi.fn().mockResolvedValue(undefined);
    const { getAllByRole, getByText } = render(
      <AttentionList workers={workers} onClose={() => undefined} onOpen={onOpen} onCopy={onCopy} />,
    );

    expect(getByText(workers[0].displayName)).toBeInTheDocument();
    fireEvent.click(getAllByRole("button", { name: "Open terminal" })[0]);
    fireEvent.click(getAllByRole("button", { name: "Copy command" })[1]);

    await waitFor(() => expect(onOpen).toHaveBeenCalledWith(workers[0]));
    expect(onCopy).toHaveBeenCalledWith(workers[1]);
  });
});
