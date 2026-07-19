import type { VideoSpriteWorkspaceSnapshot } from "../../workspace/types";
import type { VideoSpritePage } from "./video-page";

export const videoSpriteWorkspaceMethods = {
  createWorkspaceSnapshot(): VideoSpriteWorkspaceSnapshot {
    return {
      sourcePath: this.sourcePath,
      sourceName: this.sourceName,
      currentTimeSeconds: this.currentTimeSeconds,
      viewMode: this.viewMode,
      currentFrameIndex: this.currentFrameIndex,
      outputOrigin: this.processedFramesOrigin,
      savedResultPath: this.savedResult?.file_path || "",
      savedResultName: this.savedResult?.file_name || "",
      videoGeneration: {
        prompt: this.els.videoPrompt.value,
        size: this.els.videoSize.value,
        seconds: this.els.videoSeconds.value,
        sourceId: this.els.videoSourceId.value,
        direction: this.els.videoDirection.value,
        referenceImagePath: this.videoReferencePath,
        referenceImageName: this.videoReferencePath ? this.els.videoReferenceName.value : "",
        constraints: this.readVideoGenerationConstraints(),
      },
      extraction: {
        frameCount: this.els.frameCount.value,
        cols: this.els.cols.value,
        start: this.els.start.value,
        end: this.els.end.value,
        frameEdge: this.els.frameEdge.value,
        padding: this.els.padding.value,
        sourcePreviewFps: this.els.sourcePreviewFps.value,
        sourcePreviewMax: this.els.sourcePreviewMax.value,
        cropRegion: this.cropRegion ? { ...this.cropRegion } : null,
        backgroundMode: this.els.bgMode.value as VideoSpriteWorkspaceSnapshot["extraction"]["backgroundMode"],
        threshold: this.els.threshold.value,
        autoTrim: this.els.autoTrim.checked,
        transparent: this.els.transparent.checked,
        playbackFps: this.els.fps.value,
      },
      redraw: {
        finalCols: this.els.redrawFinalCols.value,
        groupRows: this.els.redrawGroupRows.value,
        groupCols: this.els.redrawGroupCols.value,
        resolution: this.els.redrawResolution.value,
        style: this.els.redrawStyle.value,
        prompt: this.els.redrawPrompt.value,
        negativePrompt: this.els.redrawNegativePrompt.value,
        constraints: this.readRedrawGenerationConstraints(),
      },
    };
  },

  async restoreWorkspaceSnapshot(snapshot: VideoSpriteWorkspaceSnapshot): Promise<void> {
    applyVideoGenerationForm(this, snapshot);
    applyExtractionForm(this, snapshot);
    applyRedrawForm(this, snapshot);

    if (snapshot.videoGeneration.referenceImagePath) {
      this.videoReferencePath = snapshot.videoGeneration.referenceImagePath;
      this.els.videoReferenceName.value = requiredText(
        snapshot.videoGeneration.referenceImageName,
        "视频参考图名称"
      );
    } else {
      this.clearVideoReference();
    }

    if (snapshot.sourcePath) {
      await this.loadSourceVideo(
        snapshot.sourcePath,
        requiredText(snapshot.sourceName, "视频源名称"),
        "恢复工作区"
      );
      applyExtractionForm(this, snapshot);
      if (!snapshot.extraction.cropRegion) {
        throw new Error("视频工作区缺少裁切区域");
      }
      this.cropRegion = { ...snapshot.extraction.cropRegion };
      this.readOptions();
      this.syncCropInputs();
    } else if (snapshot.outputOrigin !== "none") {
      throw new Error("视频工作区包含抽帧结果，但缺少源视频");
    }

    if (snapshot.outputOrigin === "source") {
      await this.handleExtract();
      if (this.processedFramesOrigin !== "source" || this.processedFrames.length === 0) {
        throw new Error("恢复视频抽帧结果失败，详细错误见视频工作台状态和日志");
      }
    }

    await this.restoreActiveRedrawRun();
    if (snapshot.outputOrigin === "redraw" && this.processedFramesOrigin !== "redraw") {
      throw new Error("工作区要求恢复 AI 重绘结果，但活动运行没有可用的最终输出");
    }
    if (snapshot.savedResultPath) {
      this.savedResult = {
        file_path: snapshot.savedResultPath,
        file_name: requiredText(snapshot.savedResultName, "已保存序列帧图名称"),
      };
    }

    if (this.processedFrames.length > 0) {
      this.currentFrameIndex = requireIndex(
        snapshot.currentFrameIndex,
        this.processedFrames.length,
        "视频输出当前帧"
      );
      this.syncTimelineToCurrentOutputFrame();
      this.updateFrameCurrentState();
    } else {
      this.currentTimeSeconds = requireFinite(snapshot.currentTimeSeconds, "视频当前时间");
      this.syncTimeControls();
    }
    this.setViewMode(snapshot.viewMode, false);
    if (this.viewMode !== snapshot.viewMode) {
      throw new Error(`无法恢复视频视图：${snapshot.viewMode}`);
    }
    this.renderCurrentView();
    this.syncControls();
    if (!this.redrawRun) this.setStatus("工作区已恢复");
  },
} satisfies ThisType<VideoSpritePage>;

function applyVideoGenerationForm(
  page: VideoSpritePage,
  snapshot: VideoSpriteWorkspaceSnapshot
): void {
  page.els.videoPrompt.value = snapshot.videoGeneration.prompt;
  setSelectValue(page.els.videoSize, snapshot.videoGeneration.size, "视频尺寸");
  page.els.videoSeconds.value = snapshot.videoGeneration.seconds;
  page.els.videoSourceId.value = snapshot.videoGeneration.sourceId;
  page.els.videoDirection.value = snapshot.videoGeneration.direction;
  const constraints = snapshot.videoGeneration.constraints;
  page.els.videoConstraintsEnabled.checked = constraints.enabled;
  setSelectValue(page.els.videoConstraintsBackground, constraints.backgroundMode, "视频约束背景模式");
  page.els.videoConstraintsBackgroundDescription.value = constraints.backgroundDescription;
  setSelectValue(page.els.videoConstraintsFraming, constraints.framing, "视频约束角色构图");
  page.els.videoConstraintsFixedCamera.checked = constraints.fixedCamera;
  page.els.videoConstraintsLoopAction.checked = constraints.loopAction;
}

function applyExtractionForm(page: VideoSpritePage, snapshot: VideoSpriteWorkspaceSnapshot): void {
  const extraction = snapshot.extraction;
  page.els.frameCount.value = extraction.frameCount;
  page.els.cols.value = extraction.cols;
  page.els.start.value = extraction.start;
  page.els.end.value = extraction.end;
  page.els.frameEdge.value = extraction.frameEdge;
  page.els.padding.value = extraction.padding;
  page.els.sourcePreviewFps.value = extraction.sourcePreviewFps;
  page.els.sourcePreviewMax.value = extraction.sourcePreviewMax;
  setSelectValue(page.els.bgMode, extraction.backgroundMode, "视频背景模式");
  page.els.threshold.value = extraction.threshold;
  page.els.autoTrim.checked = extraction.autoTrim;
  page.els.transparent.checked = extraction.transparent;
  page.els.fps.value = extraction.playbackFps;
  page.syncThresholdLabel();
  page.syncPlaybackLabels();
}

function applyRedrawForm(page: VideoSpritePage, snapshot: VideoSpriteWorkspaceSnapshot): void {
  page.els.redrawFinalCols.value = snapshot.redraw.finalCols;
  page.els.redrawGroupRows.value = snapshot.redraw.groupRows;
  page.els.redrawGroupCols.value = snapshot.redraw.groupCols;
  setSelectValue(page.els.redrawResolution, snapshot.redraw.resolution, "重绘分辨率");
  setSelectValue(page.els.redrawStyle, snapshot.redraw.style, "重绘风格");
  page.els.redrawPrompt.value = snapshot.redraw.prompt;
  page.els.redrawNegativePrompt.value = snapshot.redraw.negativePrompt;
  const constraints = snapshot.redraw.constraints;
  page.els.redrawConstraintsEnabled.checked = constraints.enabled;
  setSelectValue(page.els.redrawConstraintsBackground, constraints.backgroundMode, "重绘约束背景模式");
  page.els.redrawConstraintsBackgroundDescription.value = constraints.backgroundDescription;
  setSelectValue(page.els.redrawConstraintsFraming, constraints.framing, "重绘约束角色构图");
  page.refreshRedrawPlanSummary();
}

function setSelectValue(select: HTMLSelectElement, value: string, label: string): void {
  if (!Array.from(select.options).some((option) => option.value === value)) {
    throw new Error(`${label}快照值无效：${value}`);
  }
  select.value = value;
}

function requiredText(value: string, label: string): string {
  if (!value.trim()) throw new Error(`${label}为空`);
  return value;
}

function requireFinite(value: number, label: string): number {
  if (!Number.isFinite(value) || value < 0) throw new Error(`${label}无效`);
  return value;
}

function requireIndex(value: number, length: number, label: string): number {
  if (!Number.isInteger(value) || value < 0 || value >= length) throw new Error(`${label}无效`);
  return value;
}

export type VideoSpriteWorkspaceMethods = typeof videoSpriteWorkspaceMethods;
