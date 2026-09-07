import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { demoWorkers } from "../lib/mock";
import { Inspector } from "./Inspector";

afterEach(cleanup);

describe("Inspector", () => {
  it("shows reasoning effort and current context usage when Codex provides them", () => {
    const { getByText } = render(
      <Inspector
        worker={demoWorkers[0]}
        onClose={() => undefined}
        onOpen={vi.fn().mockResolvedValue(undefined)}
        onReviewed={vi.fn().mockResolvedValue(undefined)}
        onCopy={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(getByText("gpt-5.6-sol · high")).toBeInTheDocument();
    expect(getByText("86k / 400k (22%)")).toBeInTheDocument();
  });
});
