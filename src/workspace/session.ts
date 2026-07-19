import { getCurrentWindow } from "@tauri-apps/api/window";
import { readWorkspaceSnapshot, saveWorkspaceSnapshot } from "../api/commands";
import type { GeneratorPage } from "../features/image/image-page";
import type { SpritePage } from "../features/sprite/sprite-page";
import type { VideoSpritePage } from "../features/video/video-page";
import { getErrorMessage } from "../utils/errors";
import type { TaskCoordinator } from "../workflows/task-coordinator";
import type { WorkspaceSnapshotV3, WorkspaceTaskSnapshot } from "./types";
import { TASK_ROUTE_CHANGE_EVENT, TASK_STATE_CHANGE_EVENT } from "../workflows/events";

const SAVE_DELAY_MS = 600;
const SAVE_POLL_MS = 2_000;

export class WorkspaceSession {
  private saveTimer: number | null = null;
  private pollTimer: number | null = null;
  private saveQueue: Promise<void> = Promise.resolve();
  private lastSerializedSnapshot = "";
  private lastReportedError = "";
  private stopped = false;
  private started = false;
  private closing = false;

  constructor(
    private coordinator: TaskCoordinator,
    private generator: GeneratorPage,
    private videoSprite: VideoSpritePage,
    private sprite: SpritePage
  ) {}

  async restore(): Promise<boolean> {
    const snapshot = await readWorkspaceSnapshot();
    if (!snapshot) {
      await this.videoSprite.restoreActiveRedrawRun();
      if (this.videoSprite.redrawRun) {
        this.coordinator.restoreRoute({ kind: "video", stage: "redraw" });
        return true;
      }
      return false;
    }
    if (snapshot.schemaVersion !== 3) {
      throw new Error(`不支持的工作区版本：${snapshot.schemaVersion}`);
    }
    await this.restoreTask(snapshot.task);
    this.lastSerializedSnapshot = stableStringify(snapshot);
    this.coordinator.setSaveStatus("工作区已恢复");
    return true;
  }

  start(): void {
    if (this.started) return;
    this.started = true;
    this.stopped = false;
    document.addEventListener("input", this.handleWorkspaceEvent, true);
    document.addEventListener("change", this.handleWorkspaceEvent, true);
    document.addEventListener("click", this.handleWorkspaceEvent, true);
    document.addEventListener("pointerup", this.handleWorkspaceEvent, true);
    document.addEventListener(TASK_ROUTE_CHANGE_EVENT, this.handleWorkspaceEvent);
    document.addEventListener(TASK_STATE_CHANGE_EVENT, this.handleWorkspaceEvent);
    this.pollTimer = window.setInterval(() => this.scheduleSave(), SAVE_POLL_MS);
    this.scheduleSave();
  }

  async bindWindowClose(): Promise<void> {
    const appWindow = getCurrentWindow();
    await appWindow.onCloseRequested((event) => {
      event.preventDefault();
      if (this.closing) return;
      this.closing = true;
      this.stop();
      void this.flushForClose()
        .then(() => appWindow.destroy())
        .catch((error) => {
          this.closing = false;
          this.stopped = false;
          this.start();
          this.reportSaveError("退出前保存工作区失败", error);
        });
    });
  }

  scheduleSave = (): void => {
    if (this.stopped) return;
    if (this.saveTimer !== null) window.clearTimeout(this.saveTimer);
    if (this.coordinator.getRoute()) this.coordinator.setSaveStatus("正在保存…");
    this.saveTimer = window.setTimeout(() => {
      this.saveTimer = null;
      void this.flush(false).catch((error) => this.reportSaveError("自动保存工作区失败", error));
    }, SAVE_DELAY_MS);
  };

  flush(force: boolean): Promise<void> {
    const task = this.saveQueue.then(() => this.persist(force));
    this.saveQueue = task.catch(() => undefined);
    return task;
  }

  private flushForClose(): Promise<void> {
    const task = this.saveQueue.then(async () => {
      await this.generator.saveGenerationPreferences();
      await this.persist(true);
    });
    this.saveQueue = task.catch(() => undefined);
    return task;
  }

  private handleWorkspaceEvent = (): void => {
    this.scheduleSave();
  };

  private async persist(force: boolean): Promise<void> {
    const snapshot = await this.capture();
    const serialized = stableStringify(snapshot);
    if (!force && serialized === this.lastSerializedSnapshot) return;
    await saveWorkspaceSnapshot(snapshot);
    this.lastSerializedSnapshot = serialized;
    this.lastReportedError = "";
    this.coordinator.setSaveStatus("已自动保存");
  }

  private async capture(): Promise<WorkspaceSnapshotV3> {
    return {
      schemaVersion: 3,
      task: await this.captureTask(),
    };
  }

  private async captureTask(): Promise<WorkspaceTaskSnapshot | null> {
    const route = this.coordinator.getRoute();
    if (!route) return null;
    if (route.kind === "image") {
      return {
        ...route,
        data: {
          generator: await this.generator.createWorkspaceSnapshot(),
          sprite: this.sprite.createWorkspaceSnapshot(),
        },
      };
    }
    if (route.kind === "video") {
      return {
        ...route,
        data: {
          sourceMode: this.coordinator.getVideoSourceMode(),
          video: this.videoSprite.createWorkspaceSnapshot(),
        },
      };
    }
    return { ...route, data: this.sprite.createWorkspaceSnapshot() };
  }

  private async restoreTask(task: WorkspaceTaskSnapshot | null): Promise<void> {
    if (!task) {
      this.coordinator.restoreRoute(null);
      return;
    }
    if (task.kind === "image") {
      await this.generator.restoreWorkspaceSnapshot(task.data.generator);
      await this.sprite.restoreWorkspaceSnapshot(task.data.sprite);
      this.coordinator.restoreRoute({ kind: task.kind, stage: task.stage });
      return;
    }
    if (task.kind === "video") {
      await this.videoSprite.restoreWorkspaceSnapshot(task.data.video);
      this.coordinator.restoreRoute(
        { kind: task.kind, stage: task.stage },
        task.data.sourceMode
      );
      return;
    }
    await this.sprite.restoreWorkspaceSnapshot(task.data);
    this.coordinator.restoreRoute({ kind: task.kind, stage: task.stage });
  }

  stop(): void {
    this.started = false;
    this.stopped = true;
    if (this.saveTimer !== null) {
      window.clearTimeout(this.saveTimer);
      this.saveTimer = null;
    }
    if (this.pollTimer !== null) {
      window.clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
    document.removeEventListener("input", this.handleWorkspaceEvent, true);
    document.removeEventListener("change", this.handleWorkspaceEvent, true);
    document.removeEventListener("click", this.handleWorkspaceEvent, true);
    document.removeEventListener("pointerup", this.handleWorkspaceEvent, true);
    document.removeEventListener(TASK_ROUTE_CHANGE_EVENT, this.handleWorkspaceEvent);
    document.removeEventListener(TASK_STATE_CHANGE_EVENT, this.handleWorkspaceEvent);
  }

  async prepareForReset(): Promise<boolean> {
    const wasStarted = this.started;
    this.stop();
    await this.saveQueue;
    return wasStarted;
  }

  private reportSaveError(prefix: string, error: unknown): void {
    const message = getErrorMessage(error);
    console.error(`[workspace] ${prefix}:`, error);
    if (message === this.lastReportedError) return;
    this.lastReportedError = message;
    window.alert(`${prefix}: ${message}`);
  }
}

function stableStringify(value: unknown): string {
  return JSON.stringify(sortJsonValue(value));
}

function sortJsonValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortJsonValue);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, child]) => [key, sortJsonValue(child)])
    );
  }
  return value;
}
