import type { TaskRoute } from "./task-types";

export interface TaskPresentationContext {
  hasImageSelection: boolean;
  mattingDirty: boolean;
  hasSpriteImage: boolean;
  hasSpriteFrames: boolean;
  spritePath: string;
  videoSourceMode: "local" | "ai";
  hasVideo: boolean;
  videoName: string;
  videoOutputOrigin: "none" | "source" | "redraw";
  hasVideoOutput: boolean;
  hasRedrawRun: boolean;
}

const STAGE_HINTS: Record<string, string> = {
  "image:source": "生成或选择一张要制作成动画的图片",
  "image:matting": "可选：去除背景并修整透明区域",
  "image:grid": "确认帧数、行列和每格范围",
  "image:bounds": "可选：统一角色边界和定位针",
  "image:preview": "检查播放效果并导出帧或 GIF",
  "video:source": "选择本地视频，或切换为 AI 生成",
  "video:range": "精确设置起止时间和画面区域",
  "video:extract": "配置输出帧并执行 FFmpeg 抽帧",
  "video:redraw": "可选：按小网格分批进行一致性重绘",
  "video:preview": "检查序列帧图和动画后保存",
  "sprite:source": "导入需要切分的序列帧图片",
  "sprite:grid": "调整行列、切分区域和网格线",
  "sprite:bounds": "可选：检测或手动修改角色边界",
  "sprite:preview": "检查播放效果并导出帧或 GIF",
};

export function getStageHint(route: TaskRoute): string {
  return STAGE_HINTS[`${route.kind}:${route.stage}`];
}

export function getPrimaryLabel(route: TaskRoute, context: TaskPresentationContext): string {
  const { kind, stage } = route;
  if (kind === "image") return stage === "source"
    ? (context.hasImageSelection ? "下一步：抠图" : "开始生成")
    : stage === "matting" ? (context.mattingDirty ? "保存并继续" : "下一步：网格切分")
    : stage === "grid" ? "下一步：边界调整"
    : stage === "bounds" ? "生成预览帧"
    : "导出";
  if (kind === "sprite") return stage === "source"
    ? (context.hasSpriteImage ? "下一步：网格切分" : "选择图片")
    : stage === "grid" ? "下一步：边界调整"
    : stage === "bounds" ? "生成预览帧"
    : "导出";
  if (stage === "source") return context.hasVideo
    ? "下一步：选段裁切"
    : context.videoSourceMode === "local" ? "选择本地视频" : "AI 生成视频";
  if (stage === "range") return "确认选段";
  if (stage === "extract") return context.videoOutputOrigin === "source" ? "下一步：AI 重绘" : "抽取序列帧";
  if (stage === "redraw") return context.videoOutputOrigin === "redraw" ? "下一步：预览导出" : context.hasRedrawRun ? "继续 / 重试" : "开始分组重绘";
  return "保存 PNG";
}

export function getSecondaryLabel(route: TaskRoute): string | null {
  if (route.kind === "image" && route.stage === "matting") return "跳过抠图";
  if ((route.kind === "image" || route.kind === "sprite") && route.stage === "bounds") return "跳过边界调整";
  if (route.kind === "video" && route.stage === "redraw") return "跳过重绘";
  return null;
}

export function isTaskStepUnlocked(
  route: TaskRoute,
  stage: string,
  context: TaskPresentationContext
): boolean {
  if (stage === "source") return true;
  if (route.kind === "image") {
    if (stage === "matting" || stage === "grid") return context.hasImageSelection;
    if (stage === "bounds") return context.hasSpriteImage;
    return context.hasSpriteFrames;
  }
  if (route.kind === "sprite") {
    if (stage === "grid" || stage === "bounds") return context.hasSpriteImage;
    return context.hasSpriteFrames;
  }
  if (stage === "range" || stage === "extract") return context.hasVideo;
  if (stage === "redraw") return context.videoOutputOrigin === "source" || context.hasRedrawRun;
  return context.hasVideoOutput;
}

export function getTaskTitle(route: TaskRoute, context: TaskPresentationContext): string {
  if (route.kind === "image") return "图片动画任务";
  if (route.kind === "video") return context.videoName || "视频动画任务";
  return context.spritePath ? "序列帧编辑任务" : "序列帧图任务";
}

export function getTaskSurfaceId(route: TaskRoute): string {
  if (route.kind === "video") return "page-video-sprite";
  if (route.kind === "sprite") return "page-sprite";
  return ["grid", "bounds", "preview"].includes(route.stage)
    ? "page-sprite"
    : "page-generator";
}
