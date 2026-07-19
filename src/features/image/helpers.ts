import type { WorkbenchRecord } from "../../api/commands";
import { getFileName, stripFileExtension } from "../../utils/path";
import type { GeneratedImageRecord } from "./types";

export function formatTime(date: Date): string {
  return date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

export function parseLogTime(value: string): Date {
  const normalized = value.replace(" ", "T");
  const parsed = new Date(normalized);
  if (Number.isNaN(parsed.getTime())) {
    throw new Error(`工作台记录时间无效：${value}`);
  }
  return parsed;
}

export function workbenchDtoToRecord(dto: WorkbenchRecord): GeneratedImageRecord {
  return {
    id: dto.id,
    path: dto.path,
    label: requiredWorkbenchLabel(dto),
    prompt: dto.prompt,
    model: requiredWorkbenchModel(dto),
    durationSeconds: normalizeDurationSeconds(dto.durationSeconds),
    createdAt: parseLogTime(dto.createdAt),
    updatedAt: parseLogTime(dto.updatedAt),
  };
}

function requiredWorkbenchLabel(dto: WorkbenchRecord): string {
  const label = dto.label.trim();
  if (label) return label;
  const path = dto.path.trim() || "(空路径)";
  throw new Error(
    `工作台记录缺少标签。解决方法：请重新添加该图片，或修复工作台记录 JSON 中路径 ${path} 的 label。`
  );
}

function requiredWorkbenchModel(dto: WorkbenchRecord): string {
  const model = dto.model.trim();
  if (model) return model;
  const path = dto.path.trim() || "(空路径)";
  throw new Error(
    `工作台记录缺少模型或来源信息。解决方法：请重新添加或重新生成该图片，或修复工作台记录 JSON 中路径 ${path} 的 model。`
  );
}

export function requiredGeneratedRecordModel(record: GeneratedImageRecord, context: string): string {
  const model = record.model.trim();
  if (model) return model;
  const path = record.path.trim() || "(空路径)";
  throw new Error(
    `${context}缺少模型或来源信息。解决方法：请重新添加或重新生成该图片，或修复工作台记录 JSON 中路径 ${path} 的 model。`
  );
}

export function requiredDisplayName(name: string, path: string, context: string): string {
  const value = name.trim();
  if (value) return value;
  const displayPath = path.trim() || "(空路径)";
  throw new Error(
    `${context}缺少文件名。解决方法：请重新选择或保存带文件名的本地文件；如果这是旧数据，请确认后端返回 file_name。路径：${displayPath}`
  );
}

export function requiredPathFileStem(path: string, context: string): string {
  const fileName = getFileName(path.trim());
  const stem = fileNameToRequiredStem(fileName);
  if (stem) return stem;
  const displayPath = path.trim() || "(空路径)";
  throw new Error(
    `${context}缺少文件名。解决方法：请重新生成或重新导入带文件名的本地文件。路径：${displayPath}`
  );
}

export function requiredFileNameStem(fileName: string, path: string, context: string): string {
  const stem = fileNameToRequiredStem(fileName);
  if (stem) return stem;
  const displayPath = path.trim() || "(空路径)";
  throw new Error(
    `${context}缺少可用文件名。解决方法：请重新选择、保存或生成带文件名的本地文件；如果这是旧数据，请确认后端返回非空 file_name。路径：${displayPath}`
  );
}

export function fileNameToRequiredStem(fileName: string): string {
  return stripFileExtension(fileName.trim())
    .replace(/^[._\s-]+|[._\s-]+$/g, "")
    .trim();
}

export function recordToWorkbenchDto(record: GeneratedImageRecord): WorkbenchRecord {
  return {
    id: record.id,
    path: record.path,
    label: record.label,
    prompt: record.prompt,
    model: requiredGeneratedRecordModel(record, "待写入工作台记录"),
    durationSeconds: record.durationSeconds,
    createdAt: formatRecordTime(record.createdAt),
    updatedAt: formatRecordTime(record.updatedAt),
  };
}

function normalizeDurationSeconds(value: number | undefined): number | undefined {
  if (value === undefined) {
    return undefined;
  }
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`工作台记录耗时无效：${value}`);
  }
  return Math.round(value * 100) / 100;
}

export function formatDuration(value: number | undefined): string {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return "-";
  }
  const seconds = Math.max(0, Number(value));
  if (seconds < 60) {
    return `${seconds.toFixed(seconds < 10 ? 1 : 0)}s`;
  }
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds % 60);
  return `${minutes}m ${String(rest).padStart(2, "0")}s`;
}

export function formatRecordTime(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hour = String(date.getHours()).padStart(2, "0");
  const minute = String(date.getMinutes()).padStart(2, "0");
  const second = String(date.getSeconds()).padStart(2, "0");
  return `${year}-${month}-${day} ${hour}:${minute}:${second}`;
}

export function getEraseFailureText(reason: "outside" | "no_seed" | "erased"): string {
  switch (reason) {
    case "outside":
      return "点击位置超出图片范围";
    case "no_seed":
      return "附近没有可擦除像素";
    case "erased":
      return "点击位置已透明或没有可擦除区域";
  }
}
