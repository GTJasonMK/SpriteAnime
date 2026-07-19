import { TASK_STEPS, type TaskKind } from "./task-types";

const PENDING_TASK_KEY = "spriteanime:pending-task";
const PENDING_REFERENCE_KEY = "spriteanime:pending-reference";

export interface PendingReference {
  path: string;
  name: string;
}

export function storePendingImageTask(reference: PendingReference): void {
  sessionStorage.setItem(PENDING_TASK_KEY, "image");
  sessionStorage.setItem(PENDING_REFERENCE_KEY, JSON.stringify(reference));
}

export function consumePendingTask(): { kind: TaskKind; reference: PendingReference | null } | null {
  const kind = sessionStorage.getItem(PENDING_TASK_KEY) as TaskKind | null;
  if (!kind || !(kind in TASK_STEPS)) return null;
  sessionStorage.removeItem(PENDING_TASK_KEY);
  const rawReference = sessionStorage.getItem(PENDING_REFERENCE_KEY);
  sessionStorage.removeItem(PENDING_REFERENCE_KEY);
  return {
    kind,
    reference: kind === "image" && rawReference ? parsePendingReference(rawReference) : null,
  };
}

function parsePendingReference(raw: string): PendingReference {
  const value: unknown = JSON.parse(raw);
  if (!value || typeof value !== "object") throw new Error("待创建任务的参考图数据无效");
  const path = Reflect.get(value, "path");
  const name = Reflect.get(value, "name");
  if (typeof path !== "string" || !path.trim() || typeof name !== "string" || !name.trim()) {
    throw new Error("待创建任务的参考图路径或名称无效");
  }
  return { path, name };
}
