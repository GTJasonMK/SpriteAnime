export interface SplitRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface FrameBounds {
  index: number;
  row: number;
  col: number;
  cellX: number;
  cellY: number;
  cellWidth: number;
  cellHeight: number;
  x: number;
  y: number;
  width: number;
  height: number;
  anchorX: number;
  empty: boolean;
}

export interface AutoBoundsResult {
  rows: number;
  cols: number;
  region: SplitRegion;
  gridSignature: string;
  allowExpand: boolean;
  expandPixels: number;
  frameBounds: FrameBounds[];
  fixedOffsetX: number;
  fixedOffsetY: number;
  fixedWidth: number;
  fixedHeight: number;
  emptyCount: number;
}

export type RegionDragMode =
  | "new"
  | "move"
  | "n"
  | "s"
  | "e"
  | "w"
  | "nw"
  | "ne"
  | "sw"
  | "se";

export interface RegionDragState {
  mode: RegionDragMode;
  startX: number;
  startY: number;
  startRegion: SplitRegion;
  pointerId: number;
}

export interface GridLines {
  x: number[];
  y: number[];
}

export type GridLineAxis = "x" | "y";

export interface GridLineHit {
  axis: GridLineAxis;
  lineIndex: number;
}

export interface GridLineDragState extends GridLineHit {
  pointerId: number;
}

export interface BoundaryEditSnapshot {
  index: number;
  frame: FrameBounds;
}

export interface RgbaColor {
  r: number;
  g: number;
  b: number;
  a: number;
}
