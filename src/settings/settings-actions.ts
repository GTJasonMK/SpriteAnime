import {
  checkFfmpegTools,
  checkGenerationApi,
  checkPromptOptimizerApi,
  downloadFfmpegTools,
  exportConfig,
  importConfig,
  type ApiCheckResult,
  type FfmpegToolStatus,
} from "../api/commands";
import { getErrorMessage, isUserCancelError } from "../utils/errors";
import { bindModalFocusTrap } from "../utils/dialog";
import { getFileName } from "../utils/path";
import type { SettingsController } from "./controller";

export const settingsActionMethods = {
  bindEvents(): void {
    this.els.open.addEventListener("click", () => this.openSettings());
    this.els.close.addEventListener("click", () => void this.closeSettings());
    this.els.modal.addEventListener("click", (event) => {
      if (event.target === this.els.modal) void this.closeSettings();
    });
    bindModalFocusTrap(this.els.modal, () => void this.closeSettings());
    this.els.tabs.forEach((tab) => tab.addEventListener("click", () => this.showSettingsTab(tab.dataset.settingsTab!)));
    this.els.activeApiProfile.addEventListener("change", () => this.switchApiProfile(this.els.activeApiProfile.value));
    this.els.profileName.addEventListener("change", () => this.renameActiveApiProfile(this.els.profileName.value));
    [
      this.els.apiKey, this.els.apiBase, this.els.proxyUrl, this.els.generationApiMode, this.els.model,
      this.els.videoApiKey, this.els.videoApiBase, this.els.videoProxyUrl, this.els.videoModel, this.els.videoApiMode,
      this.els.promptOptimizerApiKey, this.els.promptOptimizerApiBase, this.els.promptOptimizerApiMode,
      this.els.promptOptimizerModel, this.els.promptOptimizerVision,
    ].forEach((control) => control.addEventListener("change", () => this.syncActiveProfile()));
    this.els.addApiProfile.addEventListener("click", () => this.addApiProfile());
    this.els.duplicateApiProfile.addEventListener("click", () => this.duplicateApiProfile());
    this.els.deleteApiProfile.addEventListener("click", () => this.deleteApiProfile());
    this.els.importConfig.addEventListener("click", () => void this.handleImportConfig());
    this.els.exportConfig.addEventListener("click", () => void this.handleExportConfig());
    this.els.toggleKey.addEventListener("click", () => this.toggleApiKeyVisibility(this.els.apiKey, this.els.toggleKey, "图片"));
    this.els.toggleVideoKey.addEventListener("click", () => this.toggleApiKeyVisibility(this.els.videoApiKey, this.els.toggleVideoKey, "视频"));
    this.els.togglePromptOptimizerKey.addEventListener("click", () => this.toggleApiKeyVisibility(this.els.promptOptimizerApiKey, this.els.togglePromptOptimizerKey, "优化"));
    this.els.copyImageApiToVideo.addEventListener("click", () => this.copyImageApiConnection("video"));
    this.els.copyImageApiToOptimizer.addEventListener("click", () => this.copyImageApiConnection("optimizer"));
    this.els.checkGenerationApi.addEventListener("click", () => void this.handleCheckGenerationApi());
    this.els.checkVideoApi.addEventListener("click", () => void this.handleCheckVideoApi());
    this.els.checkPromptOptimizerApi.addEventListener("click", () => void this.handleCheckPromptOptimizerApi());
    this.els.checkFfmpegTools.addEventListener("click", () => void this.handleCheckFfmpegTools());
    this.els.downloadFfmpegTools.addEventListener("click", () => void this.handleDownloadFfmpegTools());
    this.els.save.addEventListener("click", () => void this.handleSave());
  },

  openSettings(): void {
    this.hideApiKeys();
    const activeTab = this.els.tabs.find((tab) => tab.classList.contains("active"));
    if (!activeTab) throw new Error("设置页缺少活动标签");
    this.showSettingsTab(activeTab.dataset.settingsTab!);
    this.els.modal.style.display = "flex";
    this.els.close.focus();
  },

  async closeSettings(): Promise<void> {
    if (!await this.handleSave()) return;
    this.els.modal.style.display = "none";
    this.els.open.focus();
  },

  showSettingsTab(name: string): void {
    if (!this.els.tabs.some((tab) => tab.dataset.settingsTab === name)) throw new Error(`设置标签不存在：${name}`);
    this.els.tabs.forEach((tab) => {
      const active = tab.dataset.settingsTab === name;
      tab.classList.toggle("active", active);
      tab.setAttribute("aria-selected", String(active));
    });
    this.els.panels.forEach((panel) => {
      const active = panel.dataset.settingsPanel === name;
      panel.classList.toggle("active", active);
      panel.hidden = !active;
    });
  },

  syncActiveProfile(): void {
    this.writeFormToActiveProfile();
    this.renderApiProfiles();
    this.notifyChanged();
  },

  switchApiProfile(profileId: string): void {
    if (!this.config.api_profiles.some((profile) => profile.id === profileId)) throw new Error(`API 配置不存在：${profileId}`);
    if (this.config.active_api_profile_id !== profileId) {
      this.writeFormToActiveProfile();
      this.config.active_api_profile_id = profileId;
      this.applyProfileToForm(this.getActiveApiProfile());
    }
    this.renderApiProfiles();
    this.setStatus(`已切换 API：${this.getActiveApiProfile().name}`);
    this.notifyChanged();
  },

  renameActiveApiProfile(name: string): void {
    const normalized = name.trim();
    if (!normalized) {
      this.els.profileName.value = this.getActiveApiProfile().name;
      this.setStatus("API 配置名称不能为空", true);
      return;
    }
    this.getActiveApiProfile().name = normalized;
    this.renderApiProfiles();
    this.notifyChanged();
  },

  addApiProfile(): void {
    this.writeFormToActiveProfile();
    const profile = this.createApiProfile(`API 配置 ${this.config.api_profiles.length + 1}`);
    this.config.api_profiles.push(profile);
    this.config.active_api_profile_id = profile.id;
    this.applyProfileToForm(profile);
    this.renderApiProfiles();
    this.setStatus("已新增 API 配置");
    this.notifyChanged();
  },

  duplicateApiProfile(): void {
    this.writeFormToActiveProfile();
    const source = this.getActiveApiProfile();
    const profile = { ...source, id: `api-${crypto.randomUUID()}`, name: `${source.name} 副本` };
    this.config.api_profiles.push(profile);
    this.config.active_api_profile_id = profile.id;
    this.applyProfileToForm(profile);
    this.renderApiProfiles();
    this.setStatus("已复制 API 配置");
    this.notifyChanged();
  },

  deleteApiProfile(): void {
    if (this.config.api_profiles.length <= 1) return;
    const active = this.getActiveApiProfile();
    if (!window.confirm(`删除 API 配置「${active.name}」？`)) return;
    const index = this.config.api_profiles.findIndex((profile) => profile.id === active.id);
    this.config.api_profiles.splice(index, 1);
    const next = this.config.api_profiles[Math.min(index, this.config.api_profiles.length - 1)];
    this.config.active_api_profile_id = next.id;
    this.applyProfileToForm(next);
    this.renderApiProfiles();
    this.setStatus("已删除 API 配置");
    this.notifyChanged();
  },

  clearApiCheckStatuses(): void {
    [this.els.generationApiCheckStatus, this.els.videoApiCheckStatus, this.els.promptOptimizerApiCheckStatus].forEach((element) => {
      element.className = "config-check-status";
      element.textContent = "";
      element.title = "";
    });
  },

  toggleApiKeyVisibility(input: HTMLInputElement, button: HTMLButtonElement, label: string): void {
    const show = input.type === "password";
    input.type = show ? "text" : "password";
    button.title = show ? "隐藏 API Key" : "显示 API Key";
    button.setAttribute("aria-label", `${show ? "隐藏" : "显示"}${label} API Key`);
    button.setAttribute("aria-pressed", String(show));
  },

  hideApiKeys(): void {
    [[this.els.apiKey, this.els.toggleKey, "图片"], [this.els.videoApiKey, this.els.toggleVideoKey, "视频"], [this.els.promptOptimizerApiKey, this.els.togglePromptOptimizerKey, "优化"]].forEach(([input, button, label]) => {
      (input as HTMLInputElement).type = "password";
      (button as HTMLButtonElement).title = "显示 API Key";
      (button as HTMLButtonElement).setAttribute("aria-label", `显示${label} API Key`);
      (button as HTMLButtonElement).setAttribute("aria-pressed", "false");
    });
  },

  copyImageApiConnection(target: "video" | "optimizer"): void {
    const key = this.els.apiKey.value.trim();
    const base = this.els.apiBase.value.trim();
    if (!key || !base) {
      this.setStatus("请先填写图片生成 API Key 和地址", true);
      return;
    }
    if (target === "video") {
      this.els.videoApiKey.value = key;
      this.els.videoApiBase.value = base;
      this.els.videoProxyUrl.value = this.els.proxyUrl.value.trim();
    } else {
      this.els.promptOptimizerApiKey.value = key;
      this.els.promptOptimizerApiBase.value = base;
    }
    this.syncActiveProfile();
    this.setStatus(target === "video" ? "已复制图片连接到视频生成" : "已复制图片连接到提示词优化");
  },

  async handleSave(): Promise<boolean> {
    try {
      await this.saveCurrentConfig();
      this.setStatus("配置已保存");
      return true;
    } catch (error) {
      this.setStatus(`保存失败：${getErrorMessage(error)}`, true);
      return false;
    }
  },

  async handleImportConfig(): Promise<void> {
    if (!window.confirm("导入配置会替换当前所有设置。继续？")) return;
    try {
      const result = await importConfig();
      this.config = result.config;
      this.applyConfig();
      this.setStatus(`配置已导入：${getFileName(result.file_path) || result.file_path}`);
      this.notifyChanged();
      document.dispatchEvent(new CustomEvent("spriteanime:settings-imported"));
    } catch (error) {
      this.setStatus(
        isUserCancelError(error) ? "已取消导入" : `导入失败：${getErrorMessage(error)}`,
        !isUserCancelError(error)
      );
    }
  },

  async handleExportConfig(): Promise<void> {
    try {
      this.writeFormToActiveProfile();
      const result = await exportConfig(this.config);
      this.setStatus(`配置已导出：${getFileName(result.file_path) || result.file_path}`);
    } catch (error) {
      this.setStatus(
        isUserCancelError(error) ? "已取消导出" : `导出失败：${getErrorMessage(error)}`,
        !isUserCancelError(error)
      );
    }
  },

  async handleCheckGenerationApi(): Promise<void> {
    await this.runApiCheck(this.els.checkGenerationApi, this.els.generationApiCheckStatus, async () => {
      await this.saveCurrentConfig();
      return checkGenerationApi(this.els.apiKey.value.trim(), this.els.apiBase.value.trim(), this.els.model.value.trim(), this.els.proxyUrl.value.trim());
    });
  },

  async handleCheckVideoApi(): Promise<void> {
    await this.runApiCheck(this.els.checkVideoApi, this.els.videoApiCheckStatus, async () => {
      await this.saveCurrentConfig();
      return checkGenerationApi(this.els.videoApiKey.value.trim(), this.els.videoApiBase.value.trim(), this.els.videoModel.value.trim(), this.els.videoProxyUrl.value.trim());
    });
  },

  async handleCheckPromptOptimizerApi(): Promise<void> {
    await this.runApiCheck(this.els.checkPromptOptimizerApi, this.els.promptOptimizerApiCheckStatus, async () => {
      await this.saveCurrentConfig();
      const settings = this.getPromptOptimizerSettings();
      return checkPromptOptimizerApi(settings.apiKey, settings.apiBase, settings.model, settings.apiMode, settings.proxyUrl);
    });
  },

  async runApiCheck(button: HTMLButtonElement, status: HTMLElement, request: () => Promise<ApiCheckResult>): Promise<void> {
    const text = button.textContent;
    button.disabled = true;
    button.textContent = "检测中";
    this.setCheckStatus(status, "checking", "正在连接 API...");
    try {
      const result = await request();
      this.setCheckStatus(status, result.status === "warning" ? "warning" : "ok", result.message, result);
    } catch (error) {
      this.setCheckStatus(status, "error", `检测失败：${getErrorMessage(error)}`);
    } finally {
      button.disabled = false;
      button.textContent = text;
    }
  },

  async handleCheckFfmpegTools(): Promise<void> {
    try {
      await this.saveCurrentConfig();
      const result = await checkFfmpegTools();
      this.setFfmpegStatus(result.available ? "ok" : "warning", result.message, result);
    } catch (error) {
      this.setFfmpegStatus("error", `检测失败：${getErrorMessage(error)}`);
    }
  },

  async handleDownloadFfmpegTools(): Promise<void> {
    if (this.isInstallingFfmpeg || !window.confirm("下载 FFmpeg/FFprobe 并写入当前配置？")) return;
    this.isInstallingFfmpeg = true;
    this.els.downloadFfmpegTools.disabled = true;
    this.setFfmpegStatus("checking", "正在下载并安装 FFmpeg...");
    try {
      const result = await downloadFfmpegTools(this.els.proxyUrl.value.trim());
      this.els.ffmpegPath.value = result.ffmpeg_path;
      this.els.ffprobePath.value = result.ffprobe_path;
      await this.saveCurrentConfig();
      const status: FfmpegToolStatus = { available: true, ffmpeg_path: result.ffmpeg_path, ffprobe_path: result.ffprobe_path, message: "FFmpeg 已安装并配置。", ffmpeg_version: result.ffmpeg_version, ffprobe_version: result.ffprobe_version };
      this.setFfmpegStatus("ok", `FFmpeg 已安装：${result.install_dir}`, status);
    } catch (error) {
      this.setFfmpegStatus("error", `安装失败：${getErrorMessage(error)}`);
    } finally {
      this.isInstallingFfmpeg = false;
      this.els.downloadFfmpegTools.disabled = false;
    }
  },

  setCheckStatus(element: HTMLElement, state: string, message: string, result?: ApiCheckResult): void {
    element.className = `config-check-status ${state}`;
    element.textContent = message;
    element.title = result ? `Endpoint: ${result.endpoint}\nModel: ${result.model}` : "";
  },

  setFfmpegStatus(state: string, message: string, result?: FfmpegToolStatus): void {
    this.els.ffmpegToolStatus.className = `config-check-status ${state}`;
    this.els.ffmpegToolStatus.textContent = message;
    this.els.ffmpegToolStatus.title = result ? `FFmpeg: ${result.ffmpeg_path}\nFFprobe: ${result.ffprobe_path}` : "";
  },

  setStatus(message: string, error = false): void {
    this.els.status.textContent = message;
    this.els.status.classList.toggle("error", error);
  },
} satisfies ThisType<SettingsController>;

export type SettingsActionMethods = typeof settingsActionMethods;
