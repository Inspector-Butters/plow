export interface FarmPosition {
  x: number;
  y: number;
}

// These anchors line up with the open soil areas in the farm artwork. A plot's
// path chooses an anchor directly, so re-sorting or adding another plot cannot
// move one that is already on screen.
const plotAnchors: FarmPosition[] = [
  { x: 23, y: 39 },
  { x: 67, y: 39 },
  { x: 30, y: 63 },
  { x: 72, y: 64 },
  { x: 47, y: 49 },
  { x: 53, y: 73 },
];

// Workers choose a stable place around their plot. Small deterministic jitter
// keeps hash collisions from producing perfectly overlapping robots.
const workerOffsets: FarmPosition[] = [
  { x: 0, y: 0 },
  { x: 12, y: 0 },
  { x: -12, y: 0 },
  { x: 0, y: 14 },
  { x: 13, y: 14 },
  { x: -13, y: 14 },
  { x: 0, y: -9 },
  { x: 14, y: -9 },
  { x: -14, y: -9 },
  { x: 21, y: 5 },
  { x: -21, y: 5 },
  { x: 21, y: 17 },
];

export function stableHash(value: string): number {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

export function plotPosition(plotId: string): FarmPosition {
  return plotAnchors[stableHash(plotId) % plotAnchors.length];
}

export function workerPosition(plotId: string, workerId: string): FarmPosition {
  const anchor = plotPosition(plotId);
  const offset = workerOffsets[stableHash(workerId) % workerOffsets.length];
  const jitterX = (stableHash(`${workerId}:x`) % 25) / 10 - 1.2;
  const jitterY = (stableHash(`${workerId}:y`) % 17) / 10 - 0.8;
  const verticalDirection = anchor.y >= 60 ? -1 : 1;
  return {
    x: Math.max(5, Math.min(95, anchor.x + offset.x + jitterX)),
    y: Math.max(25, Math.min(79, anchor.y + offset.y * verticalDirection + jitterY)),
  };
}

export function motionDelay(workerId: string): number {
  return -(stableHash(`${workerId}:motion`) % 1900) / 1000;
}
