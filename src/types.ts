export type WorkerStatus =
  | "running"
  | "waitingApproval"
  | "waitingInput"
  | "completed"
  | "failed";

export type FarmActivity = "plowing" | "watering" | "planting" | "harvesting" | "carrying";

export interface Worker {
  id: string;
  parentId: string | null;
  displayName: string;
  threadName: string;
  repoName: string;
  repoPath: string;
  cwd: string;
  branch: string | null;
  model: string | null;
  source: string;
  status: WorkerStatus;
  activity: FarmActivity;
  updatedAt: number;
  startedAt: number | null;
  attentionId: string | null;
}

export interface RepoPlot {
  id: string;
  name: string;
  path: string;
  workers: Worker[];
}

export type ConnectionStatus = "connecting" | "connected" | "disconnected" | "missingCodex" | "incompatible";

export interface ConnectionInfo {
  status: ConnectionStatus;
  codexVersion: string | null;
  codexPath: string | null;
  message: string;
}

export interface MonitorSnapshot {
  workers: Worker[];
  connection: ConnectionInfo;
}

export interface PlowSettings {
  notifyWhenUnfocused: boolean;
  keepInTray: boolean;
  reducedMotion: boolean;
  codexPath: string;
}

export interface AttentionItem {
  key: string;
  workerId: string;
  kind: "approval" | "input" | "completed" | "failed";
  title: string;
  body: string;
}
