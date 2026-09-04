import type { DownloadEvent, Update } from "@tauri-apps/plugin-updater";
import { isNativeApp } from "./bridge";

export interface AppUpdateInfo {
  currentVersion: string;
  version: string;
  date: string | null;
  notes: string | null;
}

export interface AppUpdateProgress {
  phase: "downloading" | "installing" | "restarting";
  downloadedBytes: number;
  totalBytes: number | null;
}

let pendingUpdate: Update | null = null;
let pendingUpdateInfo: AppUpdateInfo | null = null;
let updateCheck: Promise<AppUpdateInfo | null> | null = null;
let dismissedVersion: string | null = null;

export function checkForAppUpdate(): Promise<AppUpdateInfo | null> {
  if (!isNativeApp()) return Promise.resolve(null);
  if (pendingUpdate && pendingUpdateInfo) return Promise.resolve(pendingUpdateInfo);
  if (updateCheck) return updateCheck;

  const currentCheck = import("@tauri-apps/plugin-updater")
    .then(({ check }) => check({ timeout: 15_000 }))
    .then(async (update) => {
      if (!update) return null;
      const info = {
        currentVersion: update.currentVersion,
        version: update.version,
        date: update.date ?? null,
        notes: update.body?.trim() || null,
      };
      if (dismissedVersion === info.version) {
        await update.close();
        return null;
      }
      pendingUpdate = update;
      pendingUpdateInfo = info;
      return info;
    });
  updateCheck = currentCheck;
  currentCheck.then(
    () => { if (updateCheck === currentCheck) updateCheck = null; },
    () => { if (updateCheck === currentCheck) updateCheck = null; },
  );
  return currentCheck;
}

export async function dismissAppUpdate(): Promise<void> {
  const update = pendingUpdate;
  dismissedVersion = pendingUpdateInfo?.version ?? null;
  pendingUpdate = null;
  pendingUpdateInfo = null;
  if (update) await update.close();
}

export async function installAppUpdate(
  onProgress: (progress: AppUpdateProgress) => void,
): Promise<void> {
  if (!pendingUpdate) throw new Error("The update is no longer available. Restart Plow to check again.");

  let downloadedBytes = 0;
  let totalBytes: number | null = null;
  await pendingUpdate.downloadAndInstall((event: DownloadEvent) => {
    if (event.event === "Started") {
      downloadedBytes = 0;
      totalBytes = event.data.contentLength ?? null;
      onProgress({ phase: "downloading", downloadedBytes, totalBytes });
    } else if (event.event === "Progress") {
      downloadedBytes += event.data.chunkLength;
      onProgress({ phase: "downloading", downloadedBytes, totalBytes });
    } else {
      onProgress({ phase: "installing", downloadedBytes, totalBytes });
    }
  });

  onProgress({ phase: "restarting", downloadedBytes, totalBytes });
  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}
