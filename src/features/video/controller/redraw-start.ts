import {
  buildRedrawConstraintPrompt,
  createVideoSpriteRedrawRun,
  loadActiveVideoSpriteRedrawRun,
  saveVideoSpriteRedrawBatchInput,
  type CreateRedrawRunRequest,
  type RedrawRunManifest
} from "../../../api/commands";
import { getErrorMessage } from "../../../utils/errors";
import {
  buildRedrawPlan,
  composeRedrawBatchInput,
  REDRAW_GROUP_WARNING_FRAMES,
  validateProjectedRedrawOutput,
  type RedrawPlan
} from "../redraw";

import type { VideoSpritePage } from "../video-page";
import { blobToDataUrl, createRedrawBatchImage, readRequiredIntegerInput, redrawBatchStatusLabel } from "./helpers";

export const videoSpriteRedrawStartMethods = {
  refreshRedrawPlanSummary(): void {
    const apiSettings = this.apiSettings.getActiveApiSettings();
    this.els.redrawApiProfile.textContent = `API: ${apiSettings.profileName}`;
    if (this.redrawRun) {
      const batchCount = this.redrawRun.batches.length;
      const succeeded = this.redrawRun.batches.filter((batch) => batch.status === "succeeded").length;
      const failed = this.redrawRun.batches.filter((batch) => batch.status === "failed").length;
      this.els.redrawSummary.className = `video-redraw-summary${failed > 0 ? " error" : ""}`;
      this.els.redrawSummary.textContent =
        `运行${redrawRunStatusLabel(this.redrawRun.status)} · ${this.redrawRun.totalFrames} 帧 · ` +
        `${this.redrawRun.groupRows}×${this.redrawRun.groupCols} 分 ${batchCount} 批 · ` +
        `完成 ${succeeded}/${batchCount}${failed ? ` · 失败 ${failed}` : ""} · ` +
        `最终 ${this.redrawRun.finalRows}×${this.redrawRun.finalCols}` +
        (failed ? "。可用原 API 配置重试；如需更换模型或调用方式，请删除运行后重新创建。" : "");
      return;
    }
    try {
      const plan = this.readRedrawPlan();
      const projected = validateProjectedRedrawOutput(
        plan,
        this.els.redrawResolution.value
      );
      const batchCount = plan.batches.length;
      const paddingCount = plan.batches[batchCount - 1].paddingCount;
      const finalEmptyCells = plan.finalRows * plan.finalCols - plan.totalFrames;
      const qualityWarning = plan.groupRows * plan.groupCols > REDRAW_GROUP_WARNING_FRAMES;
      this.redrawPlan = plan;
      this.els.redrawSummary.className = `video-redraw-summary${qualityWarning ? " warning" : ""}`;
      this.els.redrawSummary.textContent =
        `${plan.totalFrames} 帧 → ${batchCount} 张 ${plan.groupRows}×${plan.groupCols} 参考图 → ` +
        `${batchCount} 次顺序 API 调用 → 最终 ${plan.finalRows}×${plan.finalCols}` +
        `${paddingCount ? `；尾批复制补位 ${paddingCount} 格` : ""}` +
        `${finalEmptyCells ? `；最终透明空格 ${finalEmptyCells} 个` : ""}；` +
        `预计输出 ${projected.width}×${projected.height}；首批使用当前网格，后续批次额外参考上一批生成末帧` +
        `${qualityWarning ? "。每组超过 6 帧，可能降低复刻质量。" : ""}`;
    } catch (err) {
      this.redrawPlan = null;
      this.els.redrawSummary.className = "video-redraw-summary error";
      this.els.redrawSummary.textContent = getErrorMessage(err);
    }
    this.syncControls();
  },

  readRedrawPlan(): RedrawPlan {
    return buildRedrawPlan(
      readRequiredIntegerInput(this.els.frameCount, "抽帧数量", 2, 64),
      readRequiredIntegerInput(this.els.redrawFinalCols, "最终列数", 1, 20),
      readRequiredIntegerInput(this.els.redrawGroupRows, "分组行数", 1, 4),
      readRequiredIntegerInput(this.els.redrawGroupCols, "分组列数", 1, 4)
    );
  },

  async restoreActiveRedrawRun(): Promise<void> {
    this.redrawRun = await loadActiveVideoSpriteRedrawRun();
    if (this.redrawRun) {
      this.applyRedrawRunToForm(this.redrawRun);
      this.renderRedrawRun();
      if (this.redrawRun.status === "completed" && this.redrawRun.finalOutputPath) {
        await this.loadRedrawRunOutput(this.redrawRun);
      }
      const failed = this.redrawRun.batches.filter((batch) => batch.status === "failed").length;
      this.setStatus(
        failed > 0
          ? `已恢复上次暂停的分组重绘：${failed} 批失败。可用原 API 配置重试；如需更换模型或调用方式，请删除运行后重新创建。`
          : `已恢复上次分组重绘，当前状态：${redrawRunStatusLabel(this.redrawRun.status)}`
      );
    } else {
      this.renderRedrawRun();
    }
    this.refreshRedrawPlanSummary();
    this.syncControls();
  },

  applyRedrawRunToForm(run: RedrawRunManifest): void {
    this.els.frameCount.value = String(run.totalFrames);
    this.els.redrawFinalCols.value = String(run.finalCols);
    this.els.redrawGroupRows.value = String(run.groupRows);
    this.els.redrawGroupCols.value = String(run.groupCols);
    this.els.redrawNegativePrompt.value = run.negativePrompt;
    this.els.redrawStyle.value = run.style;
    this.els.redrawResolution.value = run.resolution;
  },

  renderRedrawRun(): void {
    const run = this.redrawRun;
    this.els.redrawBatches.innerHTML = "";
    if (!run) {
      this.els.redrawBatches.innerHTML = '<div class="placeholder-text">尚未创建重绘运行</div>';
      return;
    }
    const groupCapacity = run.groupRows * run.groupCols;
    run.batches.forEach((batch) => {
      const item = document.createElement("div");
      item.className = `video-redraw-batch ${batch.status}`;
      item.appendChild(createRedrawBatchImage(batch.inputPath, "输入"));
      const previousBatch = batch.index > 0 ? run.batches[batch.index - 1] : null;
      if (batch.index > 0 && !previousBatch) {
        throw new Error(`第 ${batch.index + 1} 批缺少上一批记录`);
      }
      const previousFramePaths = previousBatch ? previousBatch.framePaths : [];
      const continuityPath = previousFramePaths[previousFramePaths.length - 1] ?? "";
      item.appendChild(createRedrawBatchImage(continuityPath, batch.index > 0 ? "衔接" : "首批无衔接"));
      item.appendChild(createRedrawBatchImage(batch.outputPath, "输出"));
      const info = document.createElement("div");
      info.className = "video-redraw-batch-info";
      const title = document.createElement("span");
      title.className = "video-redraw-batch-title";
      const start = batch.globalStart + 1;
      const end = batch.globalStart + batch.validCount;
      title.textContent = `第 ${batch.index + 1}/${run.batches.length} 批 · 帧 ${start}-${end}`;
      const meta = document.createElement("span");
      const paddingCount = groupCapacity - batch.validCount;
      meta.textContent = `${redrawBatchStatusLabel(batch.status)}` +
        `${batch.index > 0 ? " · 参考上一批末帧" : " · 建立一致性基准"}` +
        `${paddingCount ? ` · 补位 ${paddingCount}` : ""}`;
      info.append(title, meta);
      if (batch.error) {
        const error = document.createElement("span");
        error.className = "video-redraw-batch-error";
        error.textContent = batch.error;
        info.appendChild(error);
      }
      item.appendChild(info);
      this.els.redrawBatches.appendChild(item);
    });
  },

  async handleStartRedraw(): Promise<void> {
    if (this.isBusy() || this.redrawRun) return;
    try {
      if (this.processedFramesOrigin !== "source" || this.processedFrames.length === 0) {
        throw new Error("请先从当前视频抽取并处理序列帧，再开始 AI 分组重绘");
      }
      const plan = this.readRedrawPlan();
      validateProjectedRedrawOutput(plan, this.els.redrawResolution.value);
      if (this.processedFrames.length !== plan.totalFrames) {
        throw new Error(
          `当前已处理 ${this.processedFrames.length} 帧，但参数要求 ${plan.totalFrames} 帧。请重新生成序列帧图。`
        );
      }
      const prompt = this.els.redrawPrompt.value.trim();
      if (!prompt) {
        throw new Error("重绘提示词为空");
      }
      const apiSettings = this.requireRedrawApiSettings();
      const options = this.readOptions();
      const constrainedPrompt = await buildRedrawConstraintPrompt(
        prompt,
        this.readRedrawGenerationConstraints()
      );
      const request: CreateRedrawRunRequest = {
        sourceName: this.sourceName,
        totalFrames: plan.totalFrames,
        finalCols: plan.finalCols,
        groupRows: plan.groupRows,
        groupCols: plan.groupCols,
        prompt: constrainedPrompt,
        negativePrompt: this.els.redrawNegativePrompt.value.trim(),
        style: this.els.redrawStyle.value,
        resolution: this.els.redrawResolution.value,
        api: {
          profileId: apiSettings.profileId,
          apiBase: apiSettings.apiBase,
          model: apiSettings.model,
          apiMode: apiSettings.apiMode,
        },
        extraction: {
          startSeconds: options.start,
          endSeconds: options.end,
        },
      };
      this.isRedrawing = true;
      this.showBusy(true, "正在准备分组参考图...");
      this.syncControls();
      this.redrawRun = await createVideoSpriteRedrawRun(request);
      this.renderRedrawRun();
      for (const batch of plan.batches) {
        const text = `正在准备第 ${batch.index + 1}/${plan.batches.length} 张参考图...`;
        this.setStatus(text);
        this.showBusy(true, text);
        const blob = await composeRedrawBatchInput(
          this.processedFrames,
          batch,
          plan.groupRows,
          plan.groupCols,
          options.transparent
        );
        this.redrawRun = await saveVideoSpriteRedrawBatchInput(
          this.redrawRun.id,
          batch.index,
          await blobToDataUrl(blob)
        );
        this.renderRedrawRun();
      }
      this.isRedrawing = false;
      await this.runRedrawBatches();
    } catch (err) {
      this.isRedrawing = false;
      this.setStatus(`分组重绘启动失败: ${getErrorMessage(err)}`);
      this.showBusy(false);
      this.syncControls();
    }
  },


} satisfies ThisType<VideoSpritePage>;

function redrawRunStatusLabel(status: RedrawRunManifest["status"]): string {
  switch (status) {
    case "preparing": return "准备中";
    case "ready": return "待执行";
    case "running": return "执行中";
    case "paused": return "已暂停";
    case "ready_to_finalize": return "待合成";
    case "completed": return "已完成";
  }
}

export type VideoSpriteRedrawStartMethods = typeof videoSpriteRedrawStartMethods;
