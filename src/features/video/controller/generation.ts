import { Channel } from "@tauri-apps/api/core";
import {
  buildVideoPrompt,
  generateVideo,
  importImageToLibrary,
  importVideoToLibrary,
  openImageFile,
  openVideoFile,
  type ActiveApiSettings,
  type GeneratedVideoResult,
  type VideoGenerationEvent
} from "../../../api/commands";
import { nextFrame } from "../../../utils/async";
import { getErrorMessage, isUserCancelError } from "../../../utils/errors";

import type { VideoSpritePage } from "../video-page";

export const videoSpriteGenerationMethods = {
  async handlePickVideo(): Promise<void> {
    if (this.isBusy()) return;

    try {
      const file = await openVideoFile();
      const imported = await importVideoToLibrary(file.file_path);
      await this.loadSourceVideo(
        imported.file_path,
        imported.file_name,
        "选择视频"
      );
    } catch (err) {
      if (!isUserCancelError(err)) {
        const message = getErrorMessage(err);
        console.error("[video-sprite] 选择视频失败:", err);
        await this.log(`pick video failed | ${message}`);
        this.setStatus(`选择视频失败: ${message}`);
      }
    }
  },

  async handlePickVideoReference(): Promise<void> {
    if (this.isBusy()) return;
    try {
      const file = await openImageFile();
      const imported = await importImageToLibrary(file.file_path);
      this.videoReferencePath = imported.file_path;
      this.els.videoReferenceName.value = imported.file_name;
      this.els.clearVideoReference.disabled = false;
    } catch (err) {
      if (!isUserCancelError(err)) {
        this.setStatus(`选择视频参考图失败: ${getErrorMessage(err)}`);
      }
    }
  },

  clearVideoReference(): void {
    this.videoReferencePath = "";
    this.els.videoReferenceName.value = "无参考图";
    this.els.clearVideoReference.disabled = true;
  },

  async handleGenerateVideo(): Promise<void> {
    if (this.isBusy()) return;

    const prompt = this.els.videoPrompt.value.trim();
    if (!prompt) {
      this.setStatus("请输入视频生成提示词");
      this.els.videoPrompt.focus();
      return;
    }

    let settings: ActiveApiSettings;
    try {
      settings = this.apiSettings.getActiveApiSettings();
    } catch (err) {
      const message = getErrorMessage(err);
      console.error("[video-sprite] 视频生成配置无效:", err);
      await this.log(`ai video config invalid | ${message}`);
      this.setStatus(`视频生成配置无效: ${message}`);
      return;
    }

    const apiKey = settings.videoApiKey.trim();
    const apiBase = settings.videoApiBase.trim();
    const proxyUrl = settings.videoProxyUrl.trim();
    const model = settings.videoModel.trim();
    const apiMode = settings.videoApiMode;
    const size = this.els.videoSize.value;
    const seconds = Number(this.els.videoSeconds.value);
    if (!Number.isInteger(seconds) || seconds < 1 || seconds > 15) {
      this.setStatus("视频秒数必须是 1 到 15 之间的整数");
      this.els.videoSeconds.focus();
      return;
    }
    const sourceVideoId = this.els.videoSourceId.value.trim();
    const extensionDirection = this.els.videoDirection.value.trim();
    let constrainedPrompt: string;
    try {
      constrainedPrompt = await buildVideoPrompt(
        prompt,
        this.readVideoGenerationConstraints(),
        Boolean(this.videoReferencePath)
      );
    } catch (err) {
      this.setStatus(getErrorMessage(err));
      return;
    }
    if (
      (apiMode === "videos_edits" || apiMode === "videos_extensions") &&
      !sourceVideoId
    ) {
      this.setStatus("当前调用方式需要原视频 ID");
      this.els.videoSourceId.focus();
      return;
    }
    if (!apiKey) {
      this.setStatus("视频生成 API Key 为空。请在设置 > API 配置 > 视频生成填写视频 API Key。");
      return;
    }
    if (!apiBase) {
      this.setStatus("视频生成 API 地址为空。请在设置 > API 配置 > 视频生成填写视频 API 地址。");
      return;
    }
    if (!model) {
      this.setStatus("视频生成模型为空。请在设置 > API 配置 > 视频生成填写视频模型。");
      return;
    }

    this.stopPlayback();
    this.isGeneratingVideo = true;
    this.savedResult = null;
    this.clearGeneratedOutput();
    this.syncControls();
    this.showBusy(true, "正在调用视频生成模型...");
    this.setStatus("正在调用视频生成模型...");
    await nextFrame();

    try {
      await this.log(
        `ai video generate start | mode=${apiMode} model=${model} size=${size} seconds=${seconds} profile=${settings.profileName}`
      );
      const channel = new Channel<VideoGenerationEvent>();
      channel.onmessage = (event: VideoGenerationEvent) => this.handleVideoGenerationProgress(event);
      const usesStandardVideoApi = apiMode !== "chat_completions";
      const result = await generateVideo(channel, {
        apiKey,
        apiBase,
        proxyUrl,
        prompt: constrainedPrompt,
        model,
        apiMode,
        size,
        seconds,
        sourceVideoId:
          apiMode === "videos_edits" || apiMode === "videos_extensions"
            ? sourceVideoId
            : "",
        extensionDirection:
          apiMode === "videos_extensions" ? extensionDirection : "",
        referenceImagePath: usesStandardVideoApi ? this.videoReferencePath : "",
      });
      await this.applyGeneratedVideo(result);
      await this.log(`ai video generate ok | path=${result.file_path}`);
    } catch (err) {
      const message = getErrorMessage(err);
      console.error("[video-sprite] 视频生成失败:", err);
      await this.log(`ai video generate failed | ${message}`);
      this.setStatus(`视频生成失败: ${message}`);
    } finally {
      this.isGeneratingVideo = false;
      this.showBusy(false);
      this.syncControls();
      this.renderCurrentView();
    }
  },

  handleVideoGenerationProgress(event: VideoGenerationEvent): void {
    switch (event.event) {
      case "Submitting":
        this.setStatus("正在调用视频生成模型...");
        this.showBusy(true, "正在调用视频生成模型...");
        break;
      case "Saving":
        this.setStatus("正在保存生成视频...");
        this.showBusy(true, "正在保存生成视频...");
        break;
      case "Completed":
        this.setStatus("视频生成完成，正在加载到抽帧工作台...");
        this.showBusy(true, "正在加载生成视频...");
        break;
    }
  },

  async applyGeneratedVideo(result: GeneratedVideoResult): Promise<void> {
    await this.loadSourceVideo(result.file_path, result.file_name, "生成视频");
    this.setStatus(
      `已生成并加载视频: ${this.sourceName}，耗时 ${result.duration_seconds.toFixed(2)}s`
    );
  },


} satisfies ThisType<VideoSpritePage>;

export type VideoSpriteGenerationMethods = typeof videoSpriteGenerationMethods;
