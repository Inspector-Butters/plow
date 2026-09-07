import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { demoWorkers } from "../lib/mock";
import { workerAppearance } from "../lib/layout";
import type { FarmActivity } from "../types";
import { RobotWorker } from "./RobotWorker";

const activities: FarmActivity[] = ["plowing", "watering", "planting", "harvesting", "carrying"];

afterEach(cleanup);

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

  it("selects the worker without triggering the field's dismiss click", () => {
    const onSelect = vi.fn();
    const onFieldClick = vi.fn();
    const { getByRole } = render(
      <div onClick={onFieldClick}>
        <RobotWorker worker={demoWorkers[0]} selected={false} x={50} y={50} onSelect={onSelect} />
      </div>,
    );

    fireEvent.click(getByRole("button"));
    expect(onSelect).toHaveBeenCalledWith(demoWorkers[0]);
    expect(onFieldClick).not.toHaveBeenCalled();
  });
});
