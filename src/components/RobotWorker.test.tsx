import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { demoWorkers } from "../lib/mock";
import { workerAppearance } from "../lib/layout";
import type { FarmActivity } from "../types";
import { RobotWorker } from "./RobotWorker";

const activities: FarmActivity[] = ["plowing", "watering", "planting", "harvesting", "carrying"];

describe("RobotWorker", () => {
  it.each(activities)("renders a distinct %s work scene", (activity) => {
    const worker = { ...demoWorkers[0], activity };
    const { container, unmount } = render(
      <RobotWorker worker={worker} selected={false} x={50} y={50} onSelect={() => undefined} />,
    );

    expect(container.querySelector(`.robot__activity--${activity}`)).toBeInTheDocument();
    expect(container.querySelector(".work-prop")).toBeInTheDocument();
    unmount();
  });

  it("uses the launch folder as the visible worker name", () => {
    const { getByText } = render(
      <RobotWorker worker={demoWorkers[2]} selected={false} x={50} y={50} onSelect={() => undefined} />,
    );

    expect(getByText("beacon-chain", { selector: ".robot__label strong" })).toBeInTheDocument();
  });

  it("keeps a worker's assigned appearance stable", () => {
    const worker = demoWorkers[0];
    const { container, rerender } = render(
      <RobotWorker worker={worker} selected={false} x={50} y={50} onSelect={() => undefined} />,
    );

    expect(container.querySelector("button")).toHaveAttribute("data-appearance", workerAppearance(worker.id));
    rerender(<RobotWorker worker={worker} selected x={60} y={55} onSelect={() => undefined} />);
    expect(container.querySelector("button")).toHaveAttribute("data-appearance", workerAppearance(worker.id));
  });
});
