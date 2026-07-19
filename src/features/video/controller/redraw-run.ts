import { Channel } from "@tauri-apps/api/core";
import {
  beginVideoSpriteRedrawBatch,
  completeVideoSpriteRedrawBatch,
  discardVideoSpriteRedrawRun,
  failVideoSpriteRedrawBatch,
  finalizeVideoSpriteRedrawRun,
  generateImage,
  loadActiveVideoSpriteRedrawRun,
  pauseVideoSpriteRedrawRun,
  readFileAsBase64,
  updateVideoSpriteRedrawFinalCols,
  type ActiveApiSettings,
  type GenerateEvent,
  type RedrawRunManifest
} from "../../../api/commands";
import { mapWithConcurrency } from "../../../utils/async";
import { getErrorMessage } from "../../../utils/errors";
import { buildRedrawPlan, validateProjectedRedrawOutput } from "../redraw";

import type { ProcessedFrame, VideoSpritePage } from "../video-page";
import { base64PngToBlob, describeGenerateEvent, readRequiredIntegerInput, requiredPathFileName } from "./helpers";

export const videoSpriteRedrawRunMethods = {
  async handleContinueRedraw(): Promise<void> {
    if (this.isBusy() || !this.redrawRun) return;
    try {
      await this.runRedrawBatches();
    } catch (err) {
      this.setStatus(`继续分组重绘失败: ${getErrorMessage(err)}`);
    }
  },

  async runRedrawBatches(): Promise<void> {
    if (!this.redrawRun || this.isRedrawing) return;
    const apiSettings = this.requireMatchingRedrawApiSettings(this.redrawRun);
    this.isRedrawing = true;
    this.redrawPauseRequested = false;
    this.syncControls();
    try {
      while (this.redrawRun) {
        if (this.redrawPauseRequested) {
          this.redrawRun = await pauseVideoSpriteRedrawRun(this.redrawRun.id);
          this.setStatus("分组重绘已暂停");
          break;
        }
        const batch = this.redrawRun.batches.find(
          (item) => item.status === "failed" || item.status === "pending"
        );
        if (!batch) break;
        const execution = await beginVideoSpriteRedrawBatch(this.redrawRun.id, batch.index);
        this.redrawRun = execution.manifest;
        this.renderRedrawRun();
        const batchCount = this.redrawRun.batches.length;
        const channel = new Channel<GenerateEvent>();
        channel.onmessage = (event) => {
          const text = `重绘第 ${batch.index + 1}/${batchCount} 批：${describeGenerateEvent(event)}`;
          this.setStatus(text);
          this.showBusy(true, text);
        };
        try {
          const result = await generateImage(
            channel,
            apiSettings.apiKey,
            apiSettings.apiBase,
            execution.prompt,
            this.redrawRun.negativePrompt,
            apiSettings.model,
            this.redrawRun.style,
            `${this.redrawRun.groupCols}:${this.redrawRun.groupRows}`,
            this.redrawRun.resolution,
            1,
            apiSettings.apiMode,
            execution.referenceImagePaths,
            apiSettings.proxyUrl
          );
          if (result.image_urls.length !== 1) {
            throw new Error(`第 ${batch.index + 1} 批应返回 1 张图片，实际返回 ${result.image_urls.length} 张`);
          }
          this.redrawRun = await completeVideoSpriteRedrawBatch(
            this.redrawRun.id,
            batch.index,
            result.image_urls[0]
          );
          this.renderRedrawRun();
          this.refreshRedrawPlanSummary();
        } catch (err) {
          const message = getErrorMessage(err);
          this.redrawRun = await failVideoSpriteRedrawBatch(
            this.redrawRun.id,
            batch.index,
            message
          );
          this.renderRedrawRun();
          this.setStatus(`第 ${batch.index + 1} 批失败，运行已暂停: ${message}`);
          break;
        }
      }
      if (
        this.redrawRun &&
        this.redrawRun.batches.every((batch) => batch.status === "succeeded")
      ) {
        await this.finalizeAndLoadRedrawRun();
      }
    } finally {
      this.isRedrawing = false;
      this.showBusy(false);
      this.refreshRedrawPlanSummary();
      this.syncControls();
    }
  },

  async handlePauseRedraw(): Promise<void> {
    if (!this.redrawRun) return;
    if (this.isRedrawing) {
      this.redrawPauseRequested = true;
      this.setStatus("将在当前批次完成后暂停...");
      this.syncControls();
      return;
    }
    this.redrawRun = await pauseVideoSpriteRedrawRun(this.redrawRun.id);
    this.renderRedrawRun();
    this.refreshRedrawPlanSummary();
  },

  async handleDiscardRedraw(): Promise<void> {
    if (!this.redrawRun || this.isRedrawing) return;
    const completed = this.redrawRun.batches.filter((batch) => batch.status === "succeeded").length;
    if (!window.confirm(`删除当前重绘运行及中间文件？已完成 ${completed}/${this.redrawRun.batches.length} 批；最终已导出的 PNG 不会删除。`)) {
      return;
    }
    try {
      await discardVideoSpriteRedrawRun(this.redrawRun.id);
      this.redrawRun = null;
      this.renderRedrawRun();
      this.refreshRedrawPlanSummary();
      this.setStatus("已删除分组重绘运行");
    } catch (err) {
      this.setStatus(`删除分组重绘运行失败: ${getErrorMessage(err)}`);
    } finally {
      this.syncControls();
    }
  },

  async handleRedrawFinalColsChanged(): Promise<void> {
    this.refreshRedrawPlanSummary();
    if (!this.redrawRun || this.isRedrawing) return;
    try {
      const finalCols = readRequiredIntegerInput(this.els.redrawFinalCols, "最终列数", 1, 20);
      if (finalCols === this.redrawRun.finalCols) return;
      validateProjectedRedrawOutput(
        buildRedrawPlan(
          this.redrawRun.totalFrames,
          finalCols,
          this.redrawRun.groupRows,
          this.redrawRun.groupCols
        ),
        this.redrawRun.resolution
      );
      this.redrawRun = await updateVideoSpriteRedrawFinalCols(this.redrawRun.id, finalCols);
      this.renderRedrawRun();
      this.refreshRedrawPlanSummary();
      this.setStatus("最终列数已更新；可重新合成，无需再次调用 API");
    } catch (err) {
      this.els.redrawFinalCols.value = String(this.redrawRun.finalCols);
      this.setStatus(`更新最终列数失败: ${getErrorMessage(err)}`);
    }
  },

  async finalizeAndLoadRedrawRun(): Promise<void> {
    if (!this.redrawRun) return;
    this.setStatus("正在合成最终序列帧图...");
    this.showBusy(true, "正在合成最终序列帧图...");
    const result = await finalizeVideoSpriteRedrawRun(this.redrawRun.id);
    this.redrawRun = await loadActiveVideoSpriteRedrawRun();
    if (!this.redrawRun) {
      throw new Error("最终合成完成后无法重新读取运行清单");
    }
    await this.loadRedrawRunOutput(this.redrawRun);
    this.renderRedrawRun();
    this.setStatus(`AI 分组重绘完成并已保存: ${result.file_name}`);
  },

  async loadRedrawRunOutput(run: RedrawRunManifest): Promise<void> {
    const orderedFramePaths = run.batches
      .flatMap((batch) => batch.framePaths.map((path, localIndex) => ({
        index: batch.globalStart + localIndex,
        path,
      })))
      .sort((a, b) => a.index - b.index);
    if (orderedFramePaths.length !== run.totalFrames) {
      throw new Error(`重绘运行应有 ${run.totalFrames} 帧，实际找到 ${orderedFramePaths.length} 帧`);
    }
    const loadedFrames = await mapWithConcurrency(orderedFramePaths, 4, async (item) => {
      const blob = base64PngToBlob(await readFileAsBase64(item.path));
      const bitmap = await createImageBitmap(blob);
      const width = bitmap.width;
      const height = bitmap.height;
      bitmap.close();
      const progress = run.totalFrames <= 1 ? 0 : item.index / (run.totalFrames - 1);
      return {
        blob,
        url: URL.createObjectURL(blob),
        bitmap: null,
        time: run.extraction.startSeconds +
          (run.extraction.endSeconds - run.extraction.startSeconds) * progress,
        width,
        height,
      } satisfies ProcessedFrame;
    });
    const finalPath = run.finalOutputPath;
    if (!finalPath) {
      loadedFrames.forEach((frame) => URL.revokeObjectURL(frame.url));
      throw new Error("重绘运行缺少最终序列帧图路径");
    }
    const sheetBlob = base64PngToBlob(await readFileAsBase64(finalPath));
    const sheetBitmap = await createImageBitmap(sheetBlob);
    this.clearGeneratedOutput();
    this.processedFrames = loadedFrames;
    this.processedFramesOrigin = "redraw";
    this.spriteSheetBlob = sheetBlob;
    this.spriteSheetBitmap = sheetBitmap;
    this.savedResult = {
      file_path: finalPath,
      file_name: requiredPathFileName(finalPath, "最终序列帧图"),
    };
    this.currentFrameIndex = 0;
    this.els.frameTotal.textContent = `${loadedFrames.length} 帧`;
    this.els.size.textContent = `尺寸: ${sheetBitmap.width} x ${sheetBitmap.height}`;
    this.renderFrameList();
    this.syncPlaybackLabels();
    this.setViewMode("sheet", false);
    this.renderCurrentView();
  },

  requireRedrawApiSettings(): ActiveApiSettings {
    const settings = this.apiSettings.getActiveApiSettings();
    if (!settings.apiKey.trim()) throw new Error("图片生成 API Key 为空");
    if (!settings.apiBase.trim()) throw new Error("图片生成 API 地址为空");
    if (!settings.model.trim()) throw new Error("图片生成模型为空");
    return settings;
  },

  requireMatchingRedrawApiSettings(run: RedrawRunManifest): ActiveApiSettings {
    const settings = this.requireRedrawApiSettings();
    const mismatches: string[] = [];
    if (settings.profileId !== run.api.profileId) mismatches.push("配置组");
    if (settings.apiBase !== run.api.apiBase) mismatches.push("API 地址");
    if (settings.model !== run.api.model) mismatches.push("模型");
    if (settings.apiMode !== run.api.apiMode) mismatches.push("调用方式");
    if (mismatches.length > 0) {
      throw new Error(
        `当前图片 API 与运行快照不一致：${mismatches.join("、")}。请恢复原配置后继续，或新建运行。`
      );
    }
    return settings;
  },


} satisfies ThisType<VideoSpritePage>;

export type VideoSpriteRedrawRunMethods = typeof videoSpriteRedrawRunMethods;
