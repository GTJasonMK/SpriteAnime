import { detectSpriteLayout } from "../../api/commands";
import type { AutoBoundsResult, SplitRegion } from "./types";

interface DetectFrameBoundsOptions {
  imagePath: string;
  rows: number;
  cols: number;
  region: SplitRegion;
  bgMode: string;
  threshold: number;
  allowExpand: boolean;
  cellRects: SplitRegion[];
  gridSignature: string;
}

export function detectFrameBoundsForImageAsync(
  options: DetectFrameBoundsOptions
): Promise<AutoBoundsResult> {
  return detectSpriteLayout(
    options.imagePath,
    options.rows,
    options.cols,
    options.region,
    options.cellRects,
    options.gridSignature,
    options.bgMode,
    options.threshold,
    options.allowExpand
  );
}
