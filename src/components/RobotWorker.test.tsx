import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { demoWorkers } from "../lib/mock";
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
});
