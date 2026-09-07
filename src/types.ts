export type WorkerStatus =
  | "running"
  | "waitingApproval"
  | "waitingInput"
  | "completed"
  | "failed";

export type FarmActivity = "plowing" | "watering" | "planting" | "harvesting" | "carrying";
export type HostKind = "local" | "ssh";

export interface Worker {
  id: string;
  threadId: string;
  hostId: string;
  hostLabel: string;
  hostKind: HostKind;
  parentId: string | null;
  displayName: string;
  threadName: string;
  repoName: string;
  repoPath: string;
  cwd: string;
  branch: string | null;
  model: string | null;
  reasoningEffort: string | null;
  contextTokens: number | null;
  contextWindow: number | null;
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
  hostId: string;
  hostLabel: string;
  hostKind: HostKind;
  status: ConnectionStatus;
  codexVersion: string | null;
  codexPath: string | null;
  message: string;
}

export interface MonitorSnapshot {
  workers: Worker[];
  connections: ConnectionInfo[];
  rateLimits: HostRateLimits[];
}

export interface RateLimitWindow {
  usedPercent: number;
  windowDurationMins: number | null;
  resetsAt: number | null;
}

export interface RateLimitBucket {
  limitId: string;
  limitName: string | null;
  primary: RateLimitWindow | null;
  secondary: RateLimitWindow | null;
}

export interface HostRateLimits {
  hostId: string;
  hostLabel: string;
  limits: RateLimitBucket[];
}

export interface ProjectFolder {
  name: string;
  path: string;
}

export type AgentViewMode = "field" | "classic";

export interface SshHostSettings {
  alias: string;
  codexPath: string;
  developmentHome: string;
  enabled: boolean;
}

export interface PlowSettings {
  notifyWhenUnfocused: boolean;
  keepInTray: boolean;
  reducedMotion: boolean;
  localEnabled: boolean;
  codexPath: string;
  developmentHome: string;
  sshHosts: SshHostSettings[];
  viewMode: AgentViewMode;
}

export interface ProjectLocation {
  hostId: string;
  hostLabel: string;
  hostKind: HostKind;
  developmentHome: string;
}

export interface AttentionItem {
  key: string;
  workerId: string;
  kind: "approval" | "input" | "completed" | "failed";
  title: string;
  body: string;
}
