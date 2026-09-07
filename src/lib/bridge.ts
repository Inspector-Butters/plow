import type { MonitorSnapshot, PlowSettings, ProjectFolder } from "../types";
import { demoSnapshot } from "./mock";
import packageMetadata from "../../package.json";

export type Unlisten = () => void;

export function isNativeApp(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function getAppVersion(): Promise<string> {
  if (!isNativeApp()) return packageMetadata.version;
  const { getVersion } = await import("@tauri-apps/api/app");
  return getVersion();
}

export async function getSnapshot(): Promise<MonitorSnapshot> {
  if (!isNativeApp()) return demoSnapshot;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<MonitorSnapshot>("get_snapshot");
}

export async function subscribeToSnapshots(callback: (snapshot: MonitorSnapshot) => void): Promise<Unlisten> {
  if (!isNativeApp()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<MonitorSnapshot>("monitor-snapshot", (event) => callback(event.payload));
}

export async function markReviewed(attentionId: string): Promise<void> {
  if (!isNativeApp()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("mark_reviewed", { attentionId });
}

export async function openThread(threadId: string, cwd: string): Promise<string> {
  if (!isNativeApp()) {
    const command = resumeCommand(threadId, cwd);
    await navigator.clipboard?.writeText(command);
    return "Browser preview copied the resume command";
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string>("open_thread", { threadId });
}

export async function listProjects(hostId = "local"): Promise<ProjectFolder[]> {
  if (!isNativeApp()) {
    return [
      { name: "plow", path: "/home/demo/Developer/plow" },
      { name: "orchard-api", path: "/home/demo/Developer/orchard-api" },
      { name: "seedling", path: "/home/demo/Developer/seedling" },
    ];
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ProjectFolder[]>("list_projects", { hostId });
}

export async function startAgent(hostId: string, projectPath: string): Promise<string> {
  if (!isNativeApp()) {
    const command = hostId === "local"
      ? startAgentCommand(projectPath)
      : `ssh -t ${shellQuote(hostId.replace(/^ssh:/, ""))} codex --remote unix:// --cd ${shellQuote(projectPath)}`;
    await navigator.clipboard?.writeText(command);
    return "Browser preview copied the start command";
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string>("start_agent", { hostId, projectPath });
}

function shellQuote(value: string): string {
  return `'${value.replaceAll("'", `'\\''`)}'`;
}

export function resumeCommand(threadId: string, cwd: string): string {
  return `codex resume ${threadId} --remote unix:// --cd ${shellQuote(cwd)}`;
}

export function startAgentCommand(cwd: string): string {
  return `codex --remote unix:// --cd ${shellQuote(cwd)}`;
}

export async function copyResumeCommand(workerId: string, threadId: string, cwd: string): Promise<void> {
  let command = resumeCommand(threadId, cwd);
  if (isNativeApp()) {
    const { invoke } = await import("@tauri-apps/api/core");
    command = await invoke<string>("get_resume_command", { workerId });
  }
  await navigator.clipboard.writeText(command);
}

export async function listSshHosts(): Promise<string[]> {
  if (!isNativeApp()) return ["devbox", "build-server"];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string[]>("list_ssh_hosts");
}

export async function loadSettings(): Promise<PlowSettings> {
  const defaults: PlowSettings = {
    notifyWhenUnfocused: true,
    keepInTray: true,
    reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    localEnabled: true,
    codexPath: "",
    developmentHome: isNativeApp() ? "" : "/home/demo/Developer",
    sshHosts: [],
    viewMode: !isNativeApp() && new URLSearchParams(window.location.search).get("view") === "classic" ? "classic" : "field",
  };
  if (!isNativeApp()) return defaults;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<PlowSettings>("get_settings").catch(() => defaults);
}

export async function updateSettings(settings: PlowSettings): Promise<void> {
  if (!isNativeApp()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("update_settings", { settings });
}

export async function sendNativeNotification(title: string, body: string): Promise<void> {
  if (!isNativeApp()) return;
  const notification = await import("@tauri-apps/plugin-notification");
  let allowed = await notification.isPermissionGranted();
  if (!allowed) allowed = (await notification.requestPermission()) === "granted";
  if (allowed) notification.sendNotification({ title, body });
}

export async function isWindowFocused(): Promise<boolean> {
  if (!isNativeApp()) return true;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow().isFocused();
}
