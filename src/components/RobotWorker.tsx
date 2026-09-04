import type { CSSProperties, ReactNode } from "react";
import type { FarmActivity, Worker } from "../types";
import { motionDelay, workerAppearance } from "../lib/layout";
import { statusLabel } from "../lib/workers";

const statusIcons: Record<Worker["status"], string> = {
  running: "●",
  waitingApproval: "!",
  waitingInput: "?",
  completed: "✓",
  failed: "×",
};

const activityLabels: Record<FarmActivity, string> = {
  plowing: "Plowing the field",
  watering: "Watering seedlings",
  planting: "Planting seeds",
  harvesting: "Harvesting wheat",
  carrying: "Hauling supplies",
};

function ActivityScene({ activity }: { activity: FarmActivity }) {
  const scenes: Record<FarmActivity, ReactNode> = {
    plowing: (
      <>
        <span className="work-prop plow-tool">
          <i className="plow-tool__handle" />
          <i className="plow-tool__blade" />
        </span>
        <span className="work-effect soil-clods"><i /><i /><i /></span>
        <span className="work-effect plow-furrow"><i /><i /><i /></span>
      </>
    ),
    watering: (
      <>
        <span className="work-prop watering-can">
          <i className="watering-can__handle" />
          <i className="watering-can__spout" />
        </span>
        <span className="work-effect water-drops"><i /><i /><i /><i /><i /></span>
        <span className="work-effect water-splash"><i /><i /></span>
        <span className="work-effect tiny-flower"><i /></span>
      </>
    ),
    planting: (
      <>
        <span className="work-prop seed-pouch"><i /></span>
        <span className="work-effect falling-seeds"><i /><i /><i /></span>
        <span className="work-effect soil-mound" />
        <span className="work-effect new-sprout"><i /><i /></span>
      </>
    ),
    harvesting: (
      <>
        <span className="work-prop sickle"><i className="sickle__blade" /></span>
        <span className="work-effect wheat-stalks"><i /><i /><i /></span>
        <span className="work-effect harvest-swish" />
      </>
    ),
    carrying: (
      <span className="work-prop harvest-crate"><i /><i /><i /></span>
    ),
  };

  return <span className={`robot__activity robot__activity--${activity}`}>{scenes[activity]}</span>;
}

interface RobotWorkerProps {
  worker: Worker;
  selected: boolean;
  x: number;
  y: number;
  onSelect: (worker: Worker) => void;
}

export function RobotWorker({ worker, selected, x, y, onSelect }: RobotWorkerProps) {
  const appearance = workerAppearance(worker.id);
  const style = {
    "--worker-x": `${x}%`,
    "--worker-y": `${y}%`,
    "--motion-delay": `${motionDelay(worker.id)}s`,
  } as CSSProperties;
  const subAgent = worker.parentId !== null;

  return (
    <button
      className={`robot robot--${worker.status} robot--${worker.activity} robot--appearance-${appearance}${selected ? " robot--selected" : ""}${subAgent ? " robot--subagent" : ""}`}
      data-appearance={appearance}
      style={style}
      type="button"
      aria-label={`${worker.displayName}, ${worker.threadName}, ${statusLabel(worker.status)}, ${activityLabels[worker.activity]}`}
      aria-pressed={selected}
      onClick={() => onSelect(worker)}
    >
      <span className="robot__attention" aria-hidden="true">
        <span>{statusIcons[worker.status]}</span>
      </span>
      <span className="robot__figure" aria-hidden="true">
        <span className="robot__shadow" />
        <span className="robot__body">
          <img className="robot__sprite" src="/assets/plow-worker-v2.png" alt="" />
          <span className="robot__costume">
            <i className="robot__hat-accessory" />
            <i className="robot__chest-badge" />
            <i className="robot__boot-mark robot__boot-mark--left" />
            <i className="robot__boot-mark robot__boot-mark--right" />
          </span>
        </span>
        <ActivityScene activity={worker.activity} />
        <span className="robot__success-stars"><i>✦</i><i>✦</i><i>✦</i></span>
        <span className="robot__error-smoke"><i /><i /><i /></span>
      </span>
      <span className="robot__label">
        <span className={`status-dot status-dot--${worker.status}`} />
        <strong>{worker.displayName}</strong>
        {subAgent && <span className="robot__crew-mark">crew</span>}
      </span>
      <span className="robot__tooltip" role="tooltip">
        <strong>{worker.displayName}</strong>
        <span>{worker.threadName}</span>
        <span>{statusLabel(worker.status)} · {activityLabels[worker.activity]}</span>
        {worker.branch && <span>⌘ {worker.branch}</span>}
      </span>
    </button>
  );
}
