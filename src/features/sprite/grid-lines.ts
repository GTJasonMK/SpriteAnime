import type { GridLineAxis, GridLineHit, GridLines, SplitRegion } from "./types";
import { clampNumber } from "./utils";

const PREFERRED_MIN_CELL_SIZE = 4;

export function createEvenGridLines(rows: number, cols: number, region: SplitRegion): GridLines {
  return {
    x: createEvenOffsets(cols, region.width),
    y: createEvenOffsets(rows, region.height),
  };
}

export function normalizeGridLines(
  lines: GridLines | null,
  rows: number,
  cols: number,
  region: SplitRegion
): GridLines {
  if (!lines) {
    return createEvenGridLines(rows, cols, region);
  }

  return {
    x: normalizeAxisOffsets(lines.x, cols, region.width),
    y: normalizeAxisOffsets(lines.y, rows, region.height),
  };
}

export function createGridCellRects(
  rows: number,
  cols: number,
  region: SplitRegion,
  lines: GridLines | null
): SplitRegion[] {
  const normalized = normalizeGridLines(lines, rows, cols, region);
  const rects: SplitRegion[] = [];

  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col++) {
      const left = normalized.x[col];
      const top = normalized.y[row];
      const right = normalized.x[col + 1];
      const bottom = normalized.y[row + 1];
      rects.push({
        x: region.x + left,
        y: region.y + top,
        width: Math.max(0, right - left),
        height: Math.max(0, bottom - top),
      });
    }
  }

  return rects;
}

export function getGridCellIndexAtPoint(
  x: number,
  y: number,
  rows: number,
  cols: number,
  region: SplitRegion,
  lines: GridLines | null
): number | null {
  const normalized = normalizeGridLines(lines, rows, cols, region);
  const localX = x - region.x;
  const localY = y - region.y;

  if (
    localX < 0 ||
    localY < 0 ||
    localX >= region.width ||
    localY >= region.height
  ) {
    return null;
  }

  const col = findInterval(localX, normalized.x);
  const row = findInterval(localY, normalized.y);
  if (col < 0 || row < 0) {
    return null;
  }
  return row * cols + col;
}

export function hitTestGridLine(
  x: number,
  y: number,
  rows: number,
  cols: number,
  region: SplitRegion,
  lines: GridLines | null,
  tolerance: number
): GridLineHit | null {
  const normalized = normalizeGridLines(lines, rows, cols, region);
  const localX = x - region.x;
  const localY = y - region.y;

  if (
    localX < -tolerance ||
    localY < -tolerance ||
    localX > region.width + tolerance ||
    localY > region.height + tolerance
  ) {
    return null;
  }

  let best: { hit: GridLineHit; distance: number } | null = null;
  for (let i = 1; i < cols; i++) {
    const distance = Math.abs(localX - normalized.x[i]);
    if (distance <= tolerance && localY >= 0 && localY <= region.height) {
      best = chooseCloser(best, { axis: "x", lineIndex: i }, distance);
    }
  }
  for (let i = 1; i < rows; i++) {
    const distance = Math.abs(localY - normalized.y[i]);
    if (distance <= tolerance && localX >= 0 && localX <= region.width) {
      best = chooseCloser(best, { axis: "y", lineIndex: i }, distance);
    }
  }

  return best?.hit ?? null;
}

export function moveGridLine(
  lines: GridLines | null,
  rows: number,
  cols: number,
  region: SplitRegion,
  axis: GridLineAxis,
  lineIndex: number,
  absoluteValue: number
): GridLines {
  const normalized = normalizeGridLines(lines, rows, cols, region);
  const next: GridLines = {
    x: [...normalized.x],
    y: [...normalized.y],
  };
  const offsets = axis === "x" ? next.x : next.y;
  const divisions = axis === "x" ? cols : rows;
  const size = axis === "x" ? region.width : region.height;
  const localValue = Math.round(absoluteValue - (axis === "x" ? region.x : region.y));
  const minCellSize = getAxisMinCellSize(size, divisions);

  if (lineIndex <= 0 || lineIndex >= divisions || size < divisions) {
    return next;
  }

  offsets[lineIndex] = clampNumber(
    localValue,
    offsets[lineIndex - 1] + minCellSize,
    offsets[lineIndex + 1] - minCellSize
  );

  return next;
}

export function getGridLinesSignature(
  rows: number,
  cols: number,
  region: SplitRegion,
  lines: GridLines | null
): string {
  const normalized = normalizeGridLines(lines, rows, cols, region);
  return [
    `${rows}x${cols}`,
    `${region.x},${region.y},${region.width},${region.height}`,
    `x:${normalized.x.join(",")}`,
    `y:${normalized.y.join(",")}`,
  ].join("|");
}

export function getGridCellSizeSummary(rects: SplitRegion[]): string {
  if (rects.length === 0) {
    return "-";
  }

  let minW = Number.POSITIVE_INFINITY;
  let minH = Number.POSITIVE_INFINITY;
  let maxW = 0;
  let maxH = 0;

  rects.forEach((rect) => {
    minW = Math.min(minW, rect.width);
    minH = Math.min(minH, rect.height);
    maxW = Math.max(maxW, rect.width);
    maxH = Math.max(maxH, rect.height);
  });

  if (minW === maxW && minH === maxH) {
    return `${maxW}x${maxH}`;
  }
  return `${minW}x${minH}-${maxW}x${maxH}`;
}

export function isEvenGridLines(
  rows: number,
  cols: number,
  region: SplitRegion,
  lines: GridLines | null
): boolean {
  const normalized = normalizeGridLines(lines, rows, cols, region);
  const even = createEvenGridLines(rows, cols, region);
  return offsetsEqual(normalized.x, even.x) && offsetsEqual(normalized.y, even.y);
}

export function sameGridLineHit(a: GridLineHit | null, b: GridLineHit | null): boolean {
  return a?.axis === b?.axis && a?.lineIndex === b?.lineIndex;
}

export function cursorForGridLine(hit: GridLineHit | null): string | null {
  if (!hit) return null;
  return hit.axis === "x" ? "ew-resize" : "ns-resize";
}

function createEvenOffsets(divisions: number, size: number): number[] {
  const count = Math.max(1, divisions);
  const safeSize = Math.max(1, Math.round(size));
  const offsets: number[] = [];
  for (let i = 0; i <= count; i++) {
    offsets.push(i === count ? safeSize : Math.round((safeSize * i) / count));
  }
  return offsets;
}

function normalizeAxisOffsets(values: number[], divisions: number, size: number): number[] {
  const count = Math.max(1, divisions);
  const safeSize = Math.max(1, Math.round(size));
  const expectedLength = count + 1;
  if (values.length !== expectedLength) {
    return createEvenOffsets(count, safeSize);
  }

  const rounded = values.map((value) => Math.round(value));
  if (
    !rounded.every(Number.isFinite) ||
    rounded[0] !== 0 ||
    rounded[rounded.length - 1] !== safeSize
  ) {
    return createEvenOffsets(count, safeSize);
  }

  if (safeSize < count) {
    return createEvenOffsets(count, safeSize);
  }

  const minCellSize = getAxisMinCellSize(safeSize, count);
  const normalized = new Array<number>(expectedLength);
  normalized[0] = 0;
  normalized[count] = safeSize;
  for (let i = 1; i < count; i++) {
    const lower = normalized[i - 1] + minCellSize;
    const upper = safeSize - (count - i) * minCellSize;
    normalized[i] = clampNumber(rounded[i], lower, upper);
  }
  return normalized;
}

function getAxisMinCellSize(size: number, divisions: number): number {
  return Math.max(1, Math.min(PREFERRED_MIN_CELL_SIZE, Math.floor(size / Math.max(1, divisions))));
}

function findInterval(value: number, offsets: number[]): number {
  for (let i = 0; i < offsets.length - 1; i++) {
    if (value >= offsets[i] && value < offsets[i + 1]) {
      return i;
    }
  }
  return -1;
}

function chooseCloser(
  current: { hit: GridLineHit; distance: number } | null,
  hit: GridLineHit,
  distance: number
): { hit: GridLineHit; distance: number } {
  if (!current || distance < current.distance) {
    return { hit, distance };
  }
  return current;
}

function offsetsEqual(a: number[], b: number[]): boolean {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}
