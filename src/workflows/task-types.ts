export type TaskKind = "image" | "video" | "sprite";

export type ImageTaskStage = "source" | "matting" | "grid" | "bounds" | "preview";
export type VideoTaskStage = "source" | "range" | "extract" | "redraw" | "preview";
export type SpriteTaskStage = "source" | "grid" | "bounds" | "preview";

export type TaskRoute =
  | { kind: "image"; stage: ImageTaskStage }
  | { kind: "video"; stage: VideoTaskStage }
  | { kind: "sprite"; stage: SpriteTaskStage };

export interface TaskStep {
  id: string;
  label: string;
  optional?: boolean;
}

export const TASK_STEPS: Record<TaskKind, readonly TaskStep[]> = {
  image: [
    { id: "source", label: "图片来源" },
    { id: "matting", label: "抠图", optional: true },
    { id: "grid", label: "网格切分" },
    { id: "bounds", label: "边界调整", optional: true },
    { id: "preview", label: "预览导出" },
  ],
  video: [
    { id: "source", label: "视频来源" },
    { id: "range", label: "选段裁切" },
    { id: "extract", label: "抽帧处理" },
    { id: "redraw", label: "AI 重绘", optional: true },
    { id: "preview", label: "预览导出" },
  ],
  sprite: [
    { id: "source", label: "导入图片" },
    { id: "grid", label: "网格切分" },
    { id: "bounds", label: "边界调整", optional: true },
    { id: "preview", label: "预览导出" },
  ],
};

export function initialTaskRoute(kind: TaskKind): TaskRoute {
  return { kind, stage: "source" } as TaskRoute;
}

export function isTaskRoute(value: unknown): value is TaskRoute {
  if (!value || typeof value !== "object") return false;
  const route = value as { kind?: string; stage?: string };
  return TASK_STEPS[route.kind as TaskKind]?.some((step) => step.id === route.stage) ?? false;
}
