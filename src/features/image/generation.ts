import { Channel } from "@tauri-apps/api/core";
import {
  addPromptHistory,
  buildSpriteImagePrompt,
  generateImage,
  optimizePrompt,
  type GenerateEvent,
  type GenerationResult
} from "../../api/commands";
import { getErrorMessage } from "../../utils/errors";
import { parseClampedInt } from "../../utils/number";
import { requiredGenerationApiMode } from "../../settings/api-modes";

import type { GeneratorPage } from "./image-page";

export const generatorGenerationMethods = {
  async handleOptimizePrompt(): Promise<void> {
    if (!this.canRunGeneratorAction("optimizePrompt")) return;

    const prompt = this.els.prompt.value.trim();
    if (!prompt) {
      alert("请先输入需要优化的提示词");
      return;
    }

    const { apiKey, apiBase, apiMode, proxyUrl, model, vision } = this.settings.getPromptOptimizerSettings();
    if (!apiKey) {
      alert("请先在设置中填写提示词优化 API Key");
      return;
    }

    if (!apiBase) {
      alert("请填写提示词优化 API 地址");
      return;
    }

    if (!model) {
      alert("请填写提示词优化模型");
      return;
    }

    try {
      await this.saveGenerationPreferences();
    } catch (err) {
      console.error("[generator] 优化前保存配置失败:", err);
      alert(`保存当前 API 配置失败:\n${getErrorMessage(err)}`);
      return;
    }

    this.setWorkflowState("optimizing");
    this.els.optimizePrompt.classList.add("is-loading");
    this.els.optimizePrompt.setAttribute("aria-busy", "true");
    this.els.optimizePrompt.textContent = "优化中";
    this.els.toolbarStatus.textContent = "正在优化提示词...";
    this.els.toolbarStatus.title = "";

    try {
      const result = await optimizePrompt(
        apiKey,
        apiBase,
        apiMode,
        prompt,
        this.els.negPrompt.value.trim(),
        model,
        this.els.style.value,
        this.els.ratio.value,
        this.els.resolution.value,
        this.preferredSpriteGrid.rows,
        this.preferredSpriteGrid.cols,
        this.referenceImagePath,
        vision,
        proxyUrl
      );

      this.els.prompt.value = result.prompt.trim();
      this.els.negPrompt.value = result.negativePrompt.trim();
      this.preferredSpriteGrid = {
        rows: result.gridRows,
        cols: result.gridCols,
      };
      this.els.imageConstraintsRows.value = String(result.gridRows);
      this.els.imageConstraintsCols.value = String(result.gridCols);
      await this.saveGenerationPreferences();
      this.els.toolbarStatus.textContent = `提示词已优化 · 建议 ${this.preferredSpriteGrid.rows}x${this.preferredSpriteGrid.cols}`;
      this.els.toolbarStatus.title = "";
      this.els.prompt.focus();
    } catch (err) {
      const message = getErrorMessage(err);
      console.error("[generator] 提示词优化失败:", err);
      this.els.toolbarStatus.textContent = `提示词优化失败: ${message}`;
      this.els.toolbarStatus.title = message;
      alert(`提示词优化失败:\n${message}`);
    } finally {
      this.els.optimizePrompt.classList.remove("is-loading");
      this.els.optimizePrompt.removeAttribute("aria-busy");
      this.els.optimizePrompt.textContent = "优化提示词";
      this.settleWorkflowState();
    }
  },

  /// 核心：处理图片生成
  async handleGenerate(): Promise<void> {
    if (!this.canRunGeneratorAction("generate")) return;

    let timerStarted = false;
    try {
      const apiSettings = this.settings.getActiveApiSettings();
      const apiKey = apiSettings.apiKey.trim();
      const prompt = this.els.prompt.value.trim();

      if (!apiKey) {
        alert("请输入生图 API Key");
        return;
      }
      if (!apiSettings.apiBase.trim()) {
        alert("请输入生图 API 地址");
        return;
      }
      if (!apiSettings.model.trim()) {
        alert("请输入生图模型名称");
        return;
      }
      if (!prompt) {
        alert("请输入提示词");
        return;
      }

      this.setWorkflowState("generating");
      this.showProgress(true);
      this.startGenerationTimer();
      timerStarted = true;

      // 构建参数
      const apiBase = apiSettings.apiBase.trim();
      const negPrompt = this.els.negPrompt.value.trim();
      const model = apiSettings.model.trim();
      const apiMode = requiredGenerationApiMode(
        apiSettings.apiMode,
        "图片生成调用方式"
      );
      const style = this.els.style.value;
      const ratio = this.els.ratio.value;
      const resolution = this.els.resolution.value;
      const count = parseClampedInt(this.els.count.value, 1, 1, 4);
      const grid = {
        rows: requireGridAxis(this.els.imageConstraintsRows.value, "序列帧行数"),
        cols: requireGridAxis(this.els.imageConstraintsCols.value, "序列帧列数"),
      };
      this.preferredSpriteGrid = grid;
      const constrainedPrompt = await buildSpriteImagePrompt(
        prompt,
        this.readImageGenerationConstraints(),
        grid.rows,
        grid.cols,
        Boolean(this.referenceImagePath)
      );
      this.expectedImageCount = count;

      await this.saveGenerationPreferences();

      this.promptHistory = await addPromptHistory(prompt);
      this.historyIndex = this.promptHistory.length;
      this.settings.replacePromptHistory(this.promptHistory);

      // 创建进度通道
      const channel = new Channel<GenerateEvent>();
      channel.onmessage = (event: GenerateEvent) => {
        this.handleProgress(event);
      };

      const result: GenerationResult = await generateImage(
        channel,
        apiKey,
        apiBase,
        constrainedPrompt,
        negPrompt,
        model,
        style,
        ratio,
        resolution,
        count,
        apiMode,
        this.referenceImagePath ? [this.referenceImagePath] : [],
        apiSettings.proxyUrl.trim()
      );

      // 显示结果
      await this.addResultsToWorkbench(result, { prompt, model });

      this.els.toolbarStatus.textContent =
        `完成 ${result.image_urls.length} 张 · ${result.duration_seconds.toFixed(2)}s`;
    } catch (err) {
      console.error("[generator] 生成失败:", err);
      const elapsed = timerStarted ? this.getElapsedText() : "00:00";
      const message = getErrorMessage(err);
      if (timerStarted) {
        this.updateProgressText(`错误: ${message} · 用时 ${elapsed}`, true);
      }
      this.els.toolbarStatus.textContent = `生成失败: ${message} · 用时 ${elapsed}`;
      this.els.toolbarStatus.title = message;
    } finally {
      if (timerStarted) {
        this.stopGenerationTimer();
      }
      this.settleWorkflowState();
    }
  },

  /// 处理Channel进度事件
  handleProgress(event: GenerateEvent): void {
    switch (event.event) {
      case "SendingRequest":
        this.updateProgressBar(10);
        this.updateProgressText("正在发送请求...");
        break;
      case "ExtractingUrls":
        this.updateProgressBar(40);
        this.updateProgressText(`从响应中提取到 ${event.data.found} 张图片URL`);
        break;
      case "ProcessingImage":
        const total = Math.max(this.expectedImageCount, event.data.index, 1);
        const pct2 = 75 + ((event.data.index / total) * 20);
        this.updateProgressBar(pct2);
        this.updateProgressText(`正在处理第 ${event.data.index} 张图片 (${event.data.step})...`);
        break;
      case "Completed":
        const elapsed = this.getElapsedText();
        this.stopGenerationTimer();
        this.updateProgressBar(100);
        this.updateProgressText(
          `生成完成，共 ${event.data.total_images} 张图片，用时 ${elapsed}`
        );
        this.els.toolbarStatus.textContent = `完成 ${event.data.total_images} 张 · ${elapsed}`;
        break;
    }
  },

  showProgress(show: boolean): void {
    this.els.progressContainer.style.display = show ? "flex" : "none";
    this.els.resultActions.style.display = "none";
    if (this.generatedRecords.length === 0) {
      this.els.resultCard.style.display = "none";
    } else {
      this.els.workspaceEmpty.style.display = "none";
      this.els.resultCard.style.display = "flex";
    }
    if (show) {
      this.updateProgressBar(0);
      this.updateProgressText("准备中...");
      this.els.toolbarStatus.textContent = "生成中...";
    }
  },

  startGenerationTimer(): void {
    this.stopGenerationTimer();
    this.generationStartedAt = Date.now();
    this.lastGenerationElapsedText = "00:00";
    this.generationTimer = window.setInterval(() => {
      this.renderProgressText();
      this.els.toolbarStatus.textContent = `生成中 ${this.getElapsedText()}`;
    }, 1000);
    this.els.toolbarStatus.textContent = `生成中 ${this.getElapsedText()}`;
    this.renderProgressText();
  },

  stopGenerationTimer(): void {
    if (this.generationStartedAt !== null) {
      this.lastGenerationElapsedText = this.formatElapsedText(
        Date.now() - this.generationStartedAt
      );
    }
    if (this.generationTimer !== null) {
      window.clearInterval(this.generationTimer);
      this.generationTimer = null;
    }
    this.generationStartedAt = null;
  },

  updateProgressBar(percent: number): void {
    this.els.progressFill.style.width = `${Math.min(100, Math.max(0, percent))}%`;
  },

  updateProgressText(text: string, isError: boolean = false): void {
    this.progressBaseText = text;
    this.progressIsError = isError;
    this.renderProgressText();
    this.els.progressText.style.color = isError ? "#c6613f" : "";
  },

  renderProgressText(): void {
    if (this.generationStartedAt === null || this.progressIsError) {
      this.els.progressText.textContent = this.progressBaseText;
      return;
    }
    const elapsed = this.getElapsedText();
    this.els.progressText.textContent = `${this.progressBaseText} · 已等待 ${elapsed}`;
  },

  getElapsedText(): string {
    if (this.generationStartedAt === null) {
      return this.lastGenerationElapsedText;
    }
    this.lastGenerationElapsedText = this.formatElapsedText(
      Date.now() - this.generationStartedAt
    );
    return this.lastGenerationElapsedText;
  },

  formatElapsedText(elapsedMs: number): string {
    const elapsedSeconds = Math.max(0, Math.floor(elapsedMs / 1000));
    const minutes = Math.floor(elapsedSeconds / 60);
    const seconds = elapsedSeconds % 60;
    return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  },


} satisfies ThisType<GeneratorPage>;

export type GeneratorGenerationMethods = typeof generatorGenerationMethods;

function requireGridAxis(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > 20) {
    throw new Error(`${label}必须是 1 到 20 之间的整数`);
  }
  return parsed;
}
