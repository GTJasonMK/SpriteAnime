import { invoke } from "@tauri-apps/api/core";
import type { WorkspaceSnapshotV3 } from "../workspace/types";

export function readWorkspaceSnapshot(): Promise<WorkspaceSnapshotV3 | null> {
  return invoke<WorkspaceSnapshotV3 | null>("read_workspace_snapshot");
}

export function saveWorkspaceSnapshot(snapshot: WorkspaceSnapshotV3): Promise<void> {
  return invoke("save_workspace_snapshot", { snapshot });
}

export function saveWorkspaceImageDataUrl(slot: string, dataUrl: string): Promise<string> {
  return invoke<string>("save_workspace_image_data_url", { slot, dataUrl });
}

export function resetWorkspace(): Promise<void> {
  return invoke("reset_workspace");
}

export function revealWorkspaceSnapshot(): Promise<void> {
  return invoke("reveal_workspace_snapshot");
}
