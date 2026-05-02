import type { FrameData } from "../../api/commands";
import type { RegionDragMode, SplitRegion } from "./types";

export function normalizeGridSize(value: string, fallback: number): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.min(20, Math.max(1, parsed));
}

export function normalizeThreshold(value: string): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return 32;
  }
  return Math.min(255, Math.max(1, parsed));
}

export function getAutoExpandPixels(cellWidth: number, cellHeight: number): number {
  const shortEdge = Math.min(cellWidth, cellHeight);
  return Math.max(2, Math.round(shortEdge * 0.2));
}

export function getMinimumCellDimension(cellRects: SplitRegion[], key: "width" | "height"): number {
  if (cellRects.length === 0) {
    return 1;
  }
  return Math.max(1, Math.min(...cellRects.map((rect) => rect[key])));
}

export function parseRegionNumber(value: string, fallback: number): number {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export function clampNumber(value: number, min: number, max: number): number {
  if (max < min) return min;
  return Math.min(max, Math.max(min, value));
}

export function getRegionCenterX(x: number, width: number): number {
  return Math.round(x + width / 2);
}

export function cursorForDragMode(mode: RegionDragMode | null): string {
  switch (mode) {
    case "move":
      return "move";
    case "n":
    case "s":
      return "ns-resize";
    case "e":
    case "w":
      return "ew-resize";
    case "nw":
    case "se":
      return "nwse-resize";
    case "ne":
    case "sw":
      return "nesw-resize";
    default:
      return "crosshair";
  }
}

export function sameRegion(a: SplitRegion, b: SplitRegion): boolean {
  return (
    a.x === b.x &&
    a.y === b.y &&
    a.width === b.width &&
    a.height === b.height
  );
}

export function summarizeFrameSizes(frames: FrameData[]): string {
  if (frames.length === 0) {
    return "-";
  }

  let minW = Infinity;
  let minH = Infinity;
  let maxW = 0;
  let maxH = 0;

  frames.forEach((frame) => {
    minW = Math.min(minW, frame.width);
    minH = Math.min(minH, frame.height);
    maxW = Math.max(maxW, frame.width);
    maxH = Math.max(maxH, frame.height);
  });

  if (minW === maxW && minH === maxH) {
    return `${maxW}x${maxH}`;
  }
  return `${minW}x${minH}-${maxW}x${maxH}`;
}

export function getFileName(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  return normalized.split("/").pop() || path;
}

export function stripFileExtension(fileName: string): string {
  const index = fileName.lastIndexOf(".");
  return index > 0 ? fileName.slice(0, index) : fileName;
}

export function stripGifExtension(fileName: string): string {
  return fileName.replace(/\.gif$/i, "");
}

export function sanitizePathSegment(value: string): string {
  return value
    .trim()
    .replace(/[<>:"/\\|?*\u0000-\u001F]/g, "_")
    .replace(/\s+/g, "_")
    .replace(/_+/g, "_")
    .replace(/^[._\s-]+|[._\s-]+$/g, "");
}

export function joinPath(parent: string, child: string): string {
  const separator = parent.includes("\\") && !parent.includes("/") ? "\\" : "/";
  return parent.endsWith("/") || parent.endsWith("\\")
    ? `${parent}${child}`
    : `${parent}${separator}${child}`;
}
