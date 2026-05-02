import type { RgbaColor, SplitRegion } from "./types";
import { clampNumber } from "./utils";

export function estimateBackgroundColor(
  data: Uint8ClampedArray,
  width: number,
  height: number
): RgbaColor {
  const patch = Math.min(5, width, height);
  const corners = [
    [0, 0],
    [Math.max(0, width - patch), 0],
    [0, Math.max(0, height - patch)],
    [Math.max(0, width - patch), Math.max(0, height - patch)],
  ];
  let r = 0;
  let g = 0;
  let b = 0;
  let a = 0;
  let count = 0;

  corners.forEach(([startX, startY]) => {
    for (let y = 0; y < patch; y++) {
      for (let x = 0; x < patch; x++) {
        const px = startX + x;
        const py = startY + y;
        if (px >= width || py >= height) continue;
        const idx = (py * width + px) * 4;
        r += data[idx];
        g += data[idx + 1];
        b += data[idx + 2];
        a += data[idx + 3];
        count += 1;
      }
    }
  });

  if (count === 0) {
    return { r: 255, g: 255, b: 255, a: 255 };
  }

  return {
    r: Math.round(r / count),
    g: Math.round(g / count),
    b: Math.round(b / count),
    a: Math.round(a / count),
  };
}

export function detectContentBounds(
  data: Uint8ClampedArray,
  width: number,
  height: number,
  background: RgbaColor,
  threshold: number
): SplitRegion | null {
  let minX = width;
  let minY = height;
  let maxX = -1;
  let maxY = -1;

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const idx = (y * width + x) * 4;
      if (!isForegroundPixel(data, idx, background, threshold)) continue;
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
    }
  }

  if (maxX < minX || maxY < minY) {
    return null;
  }

  return {
    x: minX,
    y: minY,
    width: maxX - minX + 1,
    height: maxY - minY + 1,
  };
}

export function detectExpandedFrameBounds(
  data: Uint8ClampedArray,
  width: number,
  height: number,
  cellRects: SplitRegion[],
  background: RgbaColor,
  threshold: number,
  expandPixels: number
): Array<SplitRegion | null> {
  const frameCount = cellRects.length;
  const boundsByFrame = Array.from<SplitRegion | null>({ length: frameCount }).fill(null);
  if (width < 1 || height < 1) {
    return boundsByFrame;
  }

  const total = width * height;
  const foreground = new Uint8Array(total);
  const minComponentPixels = getMinimumComponentPixels(cellRects);

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const pixelIndex = y * width + x;
      if (isForegroundPixel(data, pixelIndex * 4, background, threshold)) {
        foreground[pixelIndex] = 1;
      }
    }
  }

  for (let frameIndex = 0; frameIndex < frameCount; frameIndex++) {
    boundsByFrame[frameIndex] = detectFrameOwnedBounds(
      foreground,
      width,
      height,
      cellRects,
      frameIndex,
      expandPixels,
      minComponentPixels
    );
  }

  return boundsByFrame;
}

function getMinimumComponentPixels(cellRects: SplitRegion[]): number {
  if (cellRects.length === 0) {
    return 5;
  }
  const minCellArea = Math.max(
    1,
    Math.min(...cellRects.map((cell) => Math.max(1, cell.width * cell.height)))
  );
  return Math.max(5, Math.min(24, Math.round(minCellArea * 0.00006)));
}

function detectFrameOwnedBounds(
  foreground: Uint8Array,
  imageWidth: number,
  imageHeight: number,
  cellRects: SplitRegion[],
  frameIndex: number,
  expandPixels: number,
  minComponentPixels: number
): SplitRegion | null {
  const cell = cellRects[frameIndex];
  const left = Math.round(clampNumber(cell.x - expandPixels, 0, imageWidth));
  const top = Math.round(clampNumber(cell.y - expandPixels, 0, imageHeight));
  const right = Math.round(clampNumber(cell.x + cell.width + expandPixels, left, imageWidth));
  const bottom = Math.round(clampNumber(cell.y + cell.height + expandPixels, top, imageHeight));
  const windowWidth = right - left;
  const windowHeight = bottom - top;

  if (windowWidth <= 0 || windowHeight <= 0) {
    return null;
  }

  const visited = new Uint8Array(windowWidth * windowHeight);
  const queue: number[] = [];
  let bounds: SplitRegion | null = null;

  for (let y = top; y < bottom; y++) {
    for (let x = left; x < right; x++) {
      const localIndex = (y - top) * windowWidth + (x - left);
      if (
        visited[localIndex] ||
        !foreground[y * imageWidth + x] ||
        !canStartFrameComponent(x, y, frameIndex, cellRects)
      ) {
        continue;
      }

      const component = collectOwnedComponentBounds({
        startLocalIndex: localIndex,
        left,
        top,
        right,
        bottom,
        windowWidth,
        foreground,
        visited,
        imageWidth,
        cell,
        cellRects,
        queue,
      });
      if (
        !component ||
        component.pixelCount < minComponentPixels ||
        !component.touchesCell ||
        component.ownerIndex !== frameIndex
      ) {
        continue;
      }
      bounds = unionBounds(bounds, component.bounds);
    }
  }

  return bounds;
}

interface OwnedComponentOptions {
  startLocalIndex: number;
  left: number;
  top: number;
  right: number;
  bottom: number;
  windowWidth: number;
  foreground: Uint8Array;
  visited: Uint8Array;
  imageWidth: number;
  cell: SplitRegion;
  cellRects: SplitRegion[];
  queue: number[];
}

interface OwnedComponentBounds {
  bounds: SplitRegion;
  pixelCount: number;
  touchesCell: boolean;
  ownerIndex: number;
}

function collectOwnedComponentBounds(options: OwnedComponentOptions): OwnedComponentBounds | null {
  const {
    startLocalIndex,
    left,
    top,
    right,
    bottom,
    windowWidth,
    foreground,
    visited,
    imageWidth,
    cell,
    cellRects,
    queue,
  } = options;
  let minX = right;
  let minY = bottom;
  let maxX = left - 1;
  let maxY = top - 1;
  let pixelCount = 0;
  let touchesCell = false;
  const ownerCounts = new Int32Array(cellRects.length);

  queue.length = 0;
  visited[startLocalIndex] = 1;
  queue.push(startLocalIndex);

  let head = 0;
  while (head < queue.length) {
    const localIndex = queue[head];
    head += 1;
    const localX = localIndex % windowWidth;
    const localY = Math.floor(localIndex / windowWidth);
    const x = left + localX;
    const y = top + localY;

    pixelCount += 1;
    touchesCell = touchesCell || isPointInRect(x, y, cell);
    const ownerIndex = findGridCellIndex(x, y, cellRects);
    if (ownerIndex >= 0) {
      ownerCounts[ownerIndex] += 1;
    }
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x);
    maxY = Math.max(maxY, y);

    enqueueOwnedNeighborPixels({
      x,
      y,
      localX,
      localY,
      left,
      top,
      right,
      bottom,
      windowWidth,
      foreground,
      visited,
      imageWidth,
      queue,
    });
  }

  if (maxX < minX || maxY < minY) {
    return null;
  }

  return {
    bounds: {
      x: minX,
      y: minY,
      width: maxX - minX + 1,
      height: maxY - minY + 1,
    },
    pixelCount,
    touchesCell,
    ownerIndex: getDominantOwnerIndex(ownerCounts),
  };
}

function getDominantOwnerIndex(ownerCounts: Int32Array): number {
  let bestIndex = -1;
  let bestCount = 0;
  for (let index = 0; index < ownerCounts.length; index += 1) {
    const count = ownerCounts[index];
    if (count > bestCount) {
      bestCount = count;
      bestIndex = index;
    }
  }
  return bestIndex;
}

function findGridCellIndex(x: number, y: number, cellRects: SplitRegion[]): number {
  for (let i = 0; i < cellRects.length; i++) {
    const cell = cellRects[i];
    if (
      x >= cell.x &&
      y >= cell.y &&
      x < cell.x + cell.width &&
      y < cell.y + cell.height
    ) {
      return i;
    }
  }
  return -1;
}

function canStartFrameComponent(
  x: number,
  y: number,
  frameIndex: number,
  cellRects: SplitRegion[]
): boolean {
  const owner = findGridCellIndex(x, y, cellRects);
  return owner === -1 || owner === frameIndex;
}

function isPointInRect(x: number, y: number, rect: SplitRegion): boolean {
  return (
    x >= rect.x &&
    y >= rect.y &&
    x < rect.x + rect.width &&
    y < rect.y + rect.height
  );
}

function unionBounds(current: SplitRegion | null, next: SplitRegion): SplitRegion {
  if (!current) {
    return next;
  }
  const left = Math.min(current.x, next.x);
  const top = Math.min(current.y, next.y);
  const right = Math.max(current.x + current.width, next.x + next.width);
  const bottom = Math.max(current.y + current.height, next.y + next.height);
  return {
    x: left,
    y: top,
    width: right - left,
    height: bottom - top,
  };
}

function enqueueOwnedNeighborPixels(options: {
  x: number,
  y: number,
  localX: number,
  localY: number,
  left: number,
  top: number,
  right: number,
  bottom: number,
  windowWidth: number,
  foreground: Uint8Array,
  visited: Uint8Array,
  imageWidth: number,
  queue: number[]
}): void {
  const {
    x,
    y,
    localX,
    localY,
    left,
    top,
    right,
    bottom,
    windowWidth,
    foreground,
    visited,
    imageWidth,
    queue,
  } = options;

  for (let dy = -1; dy <= 1; dy++) {
    for (let dx = -1; dx <= 1; dx++) {
      if (dx === 0 && dy === 0) {
        continue;
      }
      const nx = x + dx;
      const ny = y + dy;
      if (nx < left || ny < top || nx >= right || ny >= bottom) {
        continue;
      }
      const nextLocalIndex = (localY + dy) * windowWidth + localX + dx;
      if (
        visited[nextLocalIndex] ||
        !foreground[ny * imageWidth + nx]
      ) {
        continue;
      }
      visited[nextLocalIndex] = 1;
      queue.push(nextLocalIndex);
    }
  }
}

function isForegroundPixel(
  data: Uint8ClampedArray,
  index: number,
  background: RgbaColor,
  threshold: number
): boolean {
  const r = data[index];
  const g = data[index + 1];
  const b = data[index + 2];
  const a = data[index + 3];

  if (a <= 16) {
    return false;
  }
  if (background.a <= 16) {
    return a > 16;
  }

  const dr = r - background.r;
  const dg = g - background.g;
  const db = b - background.b;
  const colorDistance = Math.sqrt(dr * dr + dg * dg + db * db);
  const alphaDistance = Math.abs(a - background.a);
  return colorDistance > threshold || alphaDistance > threshold;
}
