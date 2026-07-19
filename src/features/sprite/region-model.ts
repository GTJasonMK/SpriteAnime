import { getGridCellIndexAtPoint } from "./grid-lines";
import type { AutoBoundsResult, GridLines, RegionDragMode, RegionDragState, SplitRegion } from "./types";
import { clampNumber } from "./utils";

interface NaturalPoint {
  x: number;
  y: number;
}

export function getFrameIndexAtPoint(
  x: number,
  y: number,
  rows: number,
  cols: number,
  region: SplitRegion,
  gridLines: GridLines | null
): number | null {
  return getGridCellIndexAtPoint(x, y, rows, cols, region, gridLines);
}

export function findBoundaryAtPoint(
  x: number,
  y: number,
  bounds: AutoBoundsResult,
  tolerance: number
): number | null {
  let bestIndex: number | null = null;
  let bestArea = Number.POSITIVE_INFINITY;

  bounds.frameBounds.forEach((frame) => {
    if (frame.empty) return;
    const left = frame.x - tolerance;
    const top = frame.y - tolerance;
    const right = frame.x + frame.width + tolerance;
    const bottom = frame.y + frame.height + tolerance;
    if (x < left || y < top || x > right || y > bottom) {
      return;
    }
    const area = frame.width * frame.height;
    if (area < bestArea) {
      bestArea = area;
      bestIndex = frame.index;
    }
  });

  return bestIndex;
}

export function getCanvasNaturalPoint(
  event: PointerEvent,
  canvas: HTMLCanvasElement,
  imageWidth: number,
  imageHeight: number
): NaturalPoint | null {
  const rect = canvas.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return null;

  return {
    x: clampNumber(((event.clientX - rect.left) / rect.width) * imageWidth, 0, imageWidth),
    y: clampNumber(((event.clientY - rect.top) / rect.height) * imageHeight, 0, imageHeight),
  };
}

export function hitTestRegion(
  x: number,
  y: number,
  region: SplitRegion | null,
  displayScale: number
): RegionDragMode | null {
  if (!region) return null;

  const tolerance = Math.max(4, 10 / Math.max(displayScale, 0.01));
  const left = region.x;
  const right = region.x + region.width;
  const top = region.y;
  const bottom = region.y + region.height;
  const nearLeft = Math.abs(x - left) <= tolerance;
  const nearRight = Math.abs(x - right) <= tolerance;
  const nearTop = Math.abs(y - top) <= tolerance;
  const nearBottom = Math.abs(y - bottom) <= tolerance;
  const insideX = x >= left - tolerance && x <= right + tolerance;
  const insideY = y >= top - tolerance && y <= bottom + tolerance;

  if (nearLeft && nearTop) return "nw";
  if (nearRight && nearTop) return "ne";
  if (nearLeft && nearBottom) return "sw";
  if (nearRight && nearBottom) return "se";
  if (nearTop && insideX) return "n";
  if (nearBottom && insideX) return "s";
  if (nearLeft && insideY) return "w";
  if (nearRight && insideY) return "e";
  if (x >= left && x <= right && y >= top && y <= bottom) return "move";
  return null;
}

export function regionFromDrag(
  pointX: number,
  pointY: number,
  rows: number,
  cols: number,
  imageWidth: number,
  imageHeight: number,
  drag: RegionDragState
): SplitRegion {
  const minW = Math.min(imageWidth, Math.max(1, cols));
  const minH = Math.min(imageHeight, Math.max(1, rows));
  const start = drag.startRegion;
  let left = start.x;
  let top = start.y;
  let right = start.x + start.width;
  let bottom = start.y + start.height;

  if (drag.mode === "move") {
    return clampSplitRegion(
      {
        x: start.x + pointX - drag.startX,
        y: start.y + pointY - drag.startY,
        width: start.width,
        height: start.height,
      },
      rows,
      cols,
      imageWidth,
      imageHeight
    );
  }

  if (drag.mode === "new") {
    left = Math.min(drag.startX, pointX);
    right = Math.max(drag.startX, pointX);
    top = Math.min(drag.startY, pointY);
    bottom = Math.max(drag.startY, pointY);
  } else {
    if (drag.mode.includes("w")) {
      left = clampNumber(pointX, 0, right - minW);
    }
    if (drag.mode.includes("e")) {
      right = clampNumber(pointX, left + minW, imageWidth);
    }
    if (drag.mode.includes("n")) {
      top = clampNumber(pointY, 0, bottom - minH);
    }
    if (drag.mode.includes("s")) {
      bottom = clampNumber(pointY, top + minH, imageHeight);
    }
  }

  left = clampNumber(left, 0, Math.max(0, imageWidth - minW));
  top = clampNumber(top, 0, Math.max(0, imageHeight - minH));
  right = clampNumber(right, left + minW, imageWidth);
  bottom = clampNumber(bottom, top + minH, imageHeight);

  return clampSplitRegion(
    {
      x: left,
      y: top,
      width: right - left,
      height: bottom - top,
    },
    rows,
    cols,
    imageWidth,
    imageHeight
  );
}

export function clampSplitRegion(
  region: SplitRegion,
  rows: number,
  cols: number,
  imageWidth: number,
  imageHeight: number
): SplitRegion {
  const minW = Math.min(imageWidth, Math.max(1, cols));
  const minH = Math.min(imageHeight, Math.max(1, rows));
  const width = clampNumber(Math.round(region.width), minW, imageWidth);
  const height = clampNumber(Math.round(region.height), minH, imageHeight);
  const x = clampNumber(Math.round(region.x), 0, Math.max(0, imageWidth - width));
  const y = clampNumber(Math.round(region.y), 0, Math.max(0, imageHeight - height));

  return { x, y, width, height };
}

export function isFullRegion(
  region: SplitRegion,
  imageWidth: number,
  imageHeight: number
): boolean {
  return (
    region.x === 0 &&
    region.y === 0 &&
    region.width === imageWidth &&
    region.height === imageHeight
  );
}
