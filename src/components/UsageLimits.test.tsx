import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { demoSnapshot } from "../lib/mock";
import { rateWindowLabel, UsageLimits } from "./UsageLimits";

afterEach(cleanup);

describe("UsageLimits", () => {
  it("formats common Codex quota windows", () => {
    expect(rateWindowLabel(300)).toBe("5h");
    expect(rateWindowLabel(10_080)).toBe("Week");
    expect(rateWindowLabel(43_200)).toBe("Month");
  });

  it("shows remaining percentages and emphasizes low limits", () => {
    const { getByLabelText } = render(<UsageLimits hosts={demoSnapshot.rateLimits} />);
    expect(getByLabelText(/5h: 63% left/)).toBeInTheDocument();
    expect(getByLabelText(/Week: 19% left/)).toHaveClass("usage-limit--low");
  });
});
