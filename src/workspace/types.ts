import type { AutoBoundsResult, BoundaryEditSnapshot, GridLines, SplitRegion } from "../features/sprite/types";
import type { BackgroundMode, PixelBounds } from "../features/video/types";
import type { VideoSpriteView } from "../features/video/video-page";
import type {
  ImageTaskStage,
  SpriteTaskStage,
  VideoTaskStage,
} from "../workflows/task-types";
import type {
  ImageGenerationConstraints,
  VideoGenerationConstraints,
} from "../generation/constraints";

export interface GeneratorWorkspaceSnapshot {
  style: string;
  ratio: string;
  resolution: string;
  count: string;
  prompt: string;
  negativePrompt: string;
  referenceImagePath: string;
  referenceImageName: string;
  selectedGeneratedPath: string | null;
  preferredSpriteGrid: { rows: number; cols: number };
  generationConstraints: ImageGenerationConstraints;
  matting: {
    active: boolean;
    dirty: boolean;
    workspaceImagePath: string;
    tolerance: string;
    feather: string;
    colorKey: string;
    clickTolerance: string;
    clickRadius: string;
  };
}

export interface VideoSpriteWorkspaceSnapshot {
  sourcePath: string;
  sourceName: string;
  currentTimeSeconds: number;
  viewMode: VideoSpriteView;
  currentFrameIndex: number;
  outputOrigin: "none" | "source" | "redraw";
  savedResultPath: string;
  savedResultName: string;
  videoGeneration: {
    prompt: string;
    size: string;
    seconds: string;
    sourceId: string;
    direction: string;
    referenceImagePath: string;
    referenceImageName: string;
    constraints: VideoGenerationConstraints;
  };
  extraction: {
    frameCount: string;
    cols: string;
    start: string;
    end: string;
    frameEdge: string;
    padding: string;
    sourcePreviewFps: string;
    sourcePreviewMax: string;
    cropRegion: PixelBounds | null;
    backgroundMode: BackgroundMode;
    threshold: string;
    autoTrim: boolean;
    transparent: boolean;
    playbackFps: string;
  };
  redraw: {
    finalCols: string;
    groupRows: string;
    groupCols: string;
    resolution: string;
    style: string;
    prompt: string;
    negativePrompt: string;
    constraints: ImageGenerationConstraints;
  };
}

export interface SpriteWorkspaceSnapshot {
  sheetImagePath: string;
  rows: string;
  cols: string;
  splitRegion: SplitRegion | null;
  gridLines: GridLines | null;
  autoTrim: boolean;
  autoExpand: boolean;
  autoBackgroundMode: string;
  autoTrimMode: string;
  autoThreshold: string;
  autoBounds: AutoBoundsResult | null;
  selectedBoundsIndex: number | null;
  boundaryEditOriginal: BoundaryEditSnapshot | null;
  boundaryEditorOpen: boolean;
  framesLoaded: boolean;
  selectedFrameIndices: number[];
  currentFramePosition: number;
  playbackFps: number;
  playbackScale: number;
}

export type WorkspaceTaskSnapshot =
  | {
      kind: "image";
      stage: ImageTaskStage;
      data: {
        generator: GeneratorWorkspaceSnapshot;
        sprite: SpriteWorkspaceSnapshot;
      };
    }
  | {
      kind: "video";
      stage: VideoTaskStage;
      data: {
        sourceMode: "local" | "ai";
        video: VideoSpriteWorkspaceSnapshot;
      };
    }
  | {
      kind: "sprite";
      stage: SpriteTaskStage;
      data: SpriteWorkspaceSnapshot;
    };

export interface WorkspaceSnapshotV3 {
  schemaVersion: 3;
  task: WorkspaceTaskSnapshot | null;
}
