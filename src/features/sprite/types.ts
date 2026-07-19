import type {
  SpriteFrameBounds as FrameBounds,
  SpriteRegion as SplitRegion,
} from "../../api/sprite";

export type {
  SpriteFrameBounds as FrameBounds,
  SpriteLayoutResult as AutoBoundsResult,
  SpriteRegion as SplitRegion,
} from "../../api/sprite";

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
