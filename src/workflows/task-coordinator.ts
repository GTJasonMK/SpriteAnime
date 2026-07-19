import { resetWorkspace } from "../api/commands";
import type { GeneratorPage } from "../features/image/image-page";
import type { SpritePage } from "../features/sprite/sprite-page";
import type { VideoSpritePage } from "../features/video/video-page";
import type { SettingsController } from "../settings/controller";
import { getById, queryAll } from "../utils/dom";
import { getErrorMessage } from "../utils/errors";
import {
  getPrimaryLabel,
  getSecondaryLabel,
  getStageHint,
  getTaskSurfaceId,
  getTaskTitle,
  isTaskStepUnlocked,
  type TaskPresentationContext,
} from "./task-presentation";
import {
  initialTaskRoute,
  TASK_STEPS,
  type TaskKind,
  type TaskRoute,
  type TaskStep,
} from "./task-types";
import { consumePendingTask, storePendingImageTask } from "./task-handoff";
import { TASK_ROUTE_CHANGE_EVENT, TASK_STATE_CHANGE_EVENT } from "./events";
import { showWorkspaceRecovery } from "./workspace-recovery";
let currentCoordinator: TaskCoordinator | null = null;
export class TaskCoordinator {
  private route: TaskRoute | null = null;
  private primaryAction: (() => Promise<void>) | null = null;
  private secondaryAction: (() => Promise<void>) | null = null;
  private videoSourceMode: "local" | "ai" = "local";
  private beforeWorkspaceReset: (() => Promise<boolean>) | null = null;
  private resumeWorkspace: (() => void) | null = null;
  private readonly els = {
    start: getById("task-start"),
    workspace: getById("unified-workspace"),
    surfaceHost: getById("workspace-surface-host"),
    stepList: getById("workspace-step-list"),
    taskTitle: getById("workspace-task-title"),
    saveStatus: getById("workspace-save-status"),
    stageTitle: getById("workspace-stage-title"),
    stageHint: getById("workspace-stage-hint"),
    primary: getById<HTMLButtonElement>("workspace-primary-action"),
    secondary: getById<HTMLButtonElement>("workspace-secondary-action"),
    newTask: getById<HTMLButtonElement>("btn-new-task"),
    localMode: getById<HTMLButtonElement>("btn-video-source-local-mode"),
    aiMode: getById<HTMLButtonElement>("btn-video-source-ai-mode"),
    localPanel: getById("video-source-local-panel"),
    aiPanel: getById("video-source-ai-panel"),
    videoSourceIdField: getById("video-generation-source-id-field"),
    videoDirectionField: getById("video-generation-direction-field"),
  };

  constructor(
    private readonly settings: SettingsController,
    private readonly generator: GeneratorPage,
    private readonly video: VideoSpritePage,
    private readonly sprite: SpritePage
  ) {
    currentCoordinator = this;
  }

  init(): void {
    this.els.surfaceHost.append(
      getById("page-generator"),
      getById("page-video-sprite"),
      getById("page-sprite")
    );
    queryAll<HTMLButtonElement>("[data-start-task]").forEach((button) => {
      button.addEventListener("click", () => {
        const kind = button.dataset.startTask as TaskKind;
        this.startTask(kind);
      });
    });
    this.els.newTask.addEventListener("click", () => void this.requestNewTask());
    this.els.primary.addEventListener("click", () => void this.runAction("primary"));
    this.els.secondary.addEventListener("click", () => void this.runAction("secondary"));
    this.els.localMode.addEventListener("click", () => void this.requestVideoSourceMode("local"));
    this.els.aiMode.addEventListener("click", () => void this.requestVideoSourceMode("ai"));
    document.addEventListener("input", () => this.refresh(), true);
    document.addEventListener("change", () => this.refresh(), true);
    document.addEventListener(TASK_STATE_CHANGE_EVENT, () => this.refresh());
    document.addEventListener("spriteanime:settings-change", () => this.refresh());
    this.setVideoSourceMode("local");
    this.render();
  }

  consumePendingTask(): boolean {
    const pending = consumePendingTask();
    if (!pending) return false;
    this.startTask(pending.kind);
    if (pending.reference) this.generator.setExternalReferenceImage(pending.reference.path, pending.reference.name);
    return true;
  }

  startTask(kind: TaskKind): void {
    this.route = initialTaskRoute(kind);
    this.render();
    this.notifyRouteChanged();
  }

  getRoute(): TaskRoute | null {
    return this.route ? { ...this.route } : null;
  }

  getVideoSourceMode(): "local" | "ai" {
    return this.videoSourceMode;
  }

  setWorkspaceResetLifecycle(
    prepare: () => Promise<boolean>,
    resume: () => void
  ): void {
    this.beforeWorkspaceReset = prepare;
    this.resumeWorkspace = resume;
  }

  restoreRoute(route: TaskRoute | null, videoSourceMode: "local" | "ai" = "local"): void {
    this.route = route ? { ...route } : null;
    this.setVideoSourceMode(videoSourceMode);
    this.render();
  }

  setSaveStatus(text: string): void {
    this.els.saveStatus.textContent = text;
  }

  showRecovery(error: unknown): void {
    showWorkspaceRecovery(error, this.beforeWorkspaceReset, this.resumeWorkspace);
  }

  async requestImageTaskWithReference(path: string, name: string): Promise<void> {
    if (!window.confirm("将当前结果设为参考图并开始新的图片任务？当前未完成工作会被清理，已保存结果会保留。")) {
      return;
    }
    await this.resetWorkspaceAndReload(() => storePendingImageTask({ path, name }));
  }

  async advanceImageToGrid(): Promise<void> {
    if (!this.generator.selectedGeneratedPath) return;
    await this.prepareSpriteFromGenerator();
    this.setRoute({ kind: "image", stage: "grid" });
  }

  private async requestNewTask(): Promise<void> {
    if (this.isBusy()) {
      window.alert("当前操作尚未完成，请等待完成或暂停后再新建任务。");
      return;
    }
    if (!window.confirm("新建任务会清除当前未完成工作、临时帧和活动重绘运行；已保存资产与导出结果会保留。继续吗？")) {
      return;
    }
    await this.resetWorkspaceAndReload();
  }

  private setVideoSourceMode(mode: "local" | "ai"): void {
    this.videoSourceMode = mode;
    this.els.localMode.classList.toggle("active", mode === "local");
    this.els.aiMode.classList.toggle("active", mode === "ai");
    this.els.localPanel.hidden = mode !== "local";
    this.els.aiPanel.hidden = mode !== "ai";
    this.refresh();
  }

  private async requestVideoSourceMode(mode: "local" | "ai"): Promise<void> {
    if (mode === this.videoSourceMode) return;
    if (this.video.sourcePath || this.video.processedFrames.length > 0 || this.video.redrawRun) {
      if (!window.confirm("切换视频来源会清除当前源视频、抽帧结果和活动重绘运行。继续吗？")) return;
      if (this.video.redrawRun) await this.video.handleDiscardRedraw();
      this.video.resetSource();
    }
    this.setVideoSourceMode(mode);
    this.notifyRouteChanged();
  }

  private async runAction(kind: "primary" | "secondary"): Promise<void> {
    const action = kind === "primary" ? this.primaryAction : this.secondaryAction;
    if (!action) return;
    try {
      await action();
    } catch (error) {
      window.alert(getErrorMessage(error));
    } finally {
      this.refresh();
    }
  }

  private setRoute(route: TaskRoute): void {
    this.route = route;
    this.render();
    this.notifyRouteChanged();
  }

  private async activateStep(step: TaskStep): Promise<void> {
    if (!this.route || !this.isStepUnlocked(step.id)) return;
    const currentIndex = this.stepIndex(this.route.stage);
    const nextIndex = this.stepIndex(step.id);
    if (nextIndex < currentIndex && this.hasDownstreamResults()) {
      const affected = TASK_STEPS[this.route.kind]
        .slice(nextIndex + 1, currentIndex + 1)
        .map((item) => item.label)
        .join("、");
      if (!window.confirm(`返回修改将使以下结果失效：${affected}。确认清理下游结果并继续吗？`)) {
        return;
      }
      await this.invalidateDownstream(step.id);
    }
    if (
      this.route.kind === "image"
      && step.id === "grid"
      && this.sprite.sheetImagePath
      && this.sprite.sheetImagePath !== this.generator.selectedGeneratedPath
      && this.sprite.frameController.hasFrames()
    ) {
      if (!window.confirm("更换源图片会清除当前网格拆分、边界和预览帧。继续吗？")) return;
      await this.invalidateDownstream("grid");
    }
    if (this.route.kind === "image" && this.route.stage === "matting" && step.id !== "matting") {
      this.generator.exitMattingMode();
    }
    await this.prepareStage(step.id);
    this.setRoute({ kind: this.route.kind, stage: step.id } as TaskRoute);
  }

  private async prepareStage(stage: string): Promise<void> {
    if (!this.route) return;
    if (this.route.kind === "image") {
      if (stage === "matting" && !this.generator.isMattingMode) await this.generator.enterMattingMode();
      if (stage === "grid" || stage === "bounds" || stage === "preview") {
        await this.prepareSpriteFromGenerator();
      }
      if (stage === "preview" && !this.sprite.frameController.hasFrames()) await this.sprite.handleLoadSplit();
    }
    if (this.route.kind === "sprite" && stage === "preview" && !this.sprite.frameController.hasFrames()) {
      await this.sprite.handleLoadSplit();
    }
    if (this.route.kind === "video") {
      if (stage === "source" || stage === "range") this.video.setViewMode("source");
      if (stage === "preview" && this.video.processedFrames.length > 0) this.video.setViewMode("sheet");
    }
  }

  private async prepareSpriteFromGenerator(): Promise<void> {
    const path = this.generator.selectedGeneratedPath;
    if (!path) throw new Error("请先选择一张图片");
    this.sprite.addImageSource(path);
    this.sprite.els.imageSelect.value = path;
    const grid = this.generator.getPreferredSpriteGrid();
    this.sprite.setGridSize(grid.rows, grid.cols, false);
    if (this.sprite.sheetImagePath !== path) await this.sprite.handlePreviewGrid();
  }

  private async invalidateDownstream(targetStage: string): Promise<void> {
    if (!this.route) return;
    if ((this.route.kind === "image" || this.route.kind === "sprite") && targetStage !== "preview") {
      this.sprite.frameController.destroyLoadedFrames();
      this.sprite.clearSplitResult("上游参数已修改，请重新拆分帧");
      this.sprite.settleWorkflowState();
    }
    if (this.route.kind === "video" && ["source", "range", "extract"].includes(targetStage)) {
      if (this.video.redrawRun) await this.video.handleDiscardRedraw();
      this.video.clearGeneratedOutput();
    }
  }

  private refresh(): void {
    if (!this.route) return;
    this.syncVideoModeFields();
    this.renderSteps();
    this.configureActions();
  }

  private syncVideoModeFields(): void {
    const mode = this.settings.getActiveVideoApiMode();
    this.els.videoSourceIdField.hidden = this.videoSourceMode !== "ai"
      || (mode !== "videos_edits" && mode !== "videos_extensions");
    this.els.videoDirectionField.hidden = this.videoSourceMode !== "ai"
      || mode !== "videos_extensions";
  }

  private render(): void {
    const active = Boolean(this.route);
    this.els.start.hidden = active;
    this.els.workspace.hidden = !active;
    this.els.newTask.hidden = !active;
    if (!this.route) {
      this.els.taskTitle.textContent = "新建动画任务";
      this.els.saveStatus.textContent = "尚未创建任务";
      return;
    }
    document.body.dataset.taskKind = this.route.kind;
    document.body.dataset.taskStage = this.route.stage;
    this.els.taskTitle.textContent = getTaskTitle(this.route, this.presentationContext());
    queryAll<HTMLElement>(".task-surface").forEach((surface) => surface.classList.remove("active"));
    getById(this.surfaceId()).classList.add("active");
    this.showStageElements();
    this.renderSteps();
    this.configureActions();
  }

  private showStageElements(): void {
    const surface = getById(this.surfaceId());
    queryAll<HTMLElement>("[data-task-stage]", surface).forEach((element) => {
      const stages = element.dataset.taskStage?.split(/\s+/) ?? [];
      element.hidden = !stages.includes(this.route!.stage);
    });
  }

  private renderSteps(): void {
    if (!this.route) return;
    const route = this.route;
    this.els.stepList.innerHTML = "";
    TASK_STEPS[route.kind].forEach((step, index) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = `workspace-step${step.id === route.stage ? " active" : ""}`;
      button.classList.toggle("completed", index < this.stepIndex(route.stage));
      button.disabled = !this.isStepUnlocked(step.id);
      button.setAttribute("aria-current", step.id === route.stage ? "step" : "false");
      button.title = button.disabled ? "请先完成前置步骤" : step.label;
      button.innerHTML = `<span class="workspace-step-index">${index < this.stepIndex(route.stage) ? "✓" : index + 1}</span><span class="workspace-step-copy"><span>${step.label}</span>${step.optional ? "<small>可选</small>" : ""}</span>`;
      button.addEventListener("click", () => void this.activateStep(step));
      this.els.stepList.appendChild(button);
    });
  }

  private configureActions(): void {
    if (!this.route) return;
    const step = TASK_STEPS[this.route.kind].find((item) => item.id === this.route!.stage)!;
    this.els.stageTitle.textContent = step.label;
    this.els.stageHint.textContent = getStageHint(this.route);
    this.primaryAction = this.buildPrimaryAction();
    this.secondaryAction = this.buildSecondaryAction();
    const primary = getPrimaryLabel(this.route, this.presentationContext());
    const secondary = getSecondaryLabel(this.route);
    this.els.primary.textContent = primary;
    this.els.primary.disabled = !this.primaryAction || this.isBusy();
    this.els.secondary.hidden = !secondary;
    this.els.secondary.textContent = secondary || "";
    this.els.secondary.disabled = !this.secondaryAction || this.isBusy();
  }

  private buildPrimaryAction(): (() => Promise<void>) | null {
    if (!this.route) return null;
    const { kind, stage } = this.route;
    if (kind === "image") {
      if (stage === "source") return this.generator.selectedGeneratedPath
        ? async () => this.activateStep(TASK_STEPS.image[1])
        : async () => this.generator.handleGenerate();
      if (stage === "matting") return async () => {
        if (this.generator.mattingDirty) await this.generator.handleSaveMattingEdits();
        await this.activateStep(TASK_STEPS.image[2]);
      };
      if (stage === "grid") return async () => this.activateStep(TASK_STEPS.image[3]);
      if (stage === "bounds") return async () => this.advanceSpriteToPreview("image");
      return async () => this.sprite.handleExport();
    }
    if (kind === "sprite") {
      if (stage === "source") return this.sprite.sheetImage
        ? async () => this.activateStep(TASK_STEPS.sprite[1])
        : async () => this.sprite.handlePickImage();
      if (stage === "grid") return async () => this.activateStep(TASK_STEPS.sprite[2]);
      if (stage === "bounds") return async () => this.advanceSpriteToPreview("sprite");
      return async () => this.sprite.handleExport();
    }
    if (stage === "source") return this.video.sourcePath
      ? async () => this.activateStep(TASK_STEPS.video[1])
      : this.videoSourceMode === "local"
        ? async () => this.video.handlePickVideo()
        : async () => this.video.handleGenerateVideo();
    if (stage === "range") return async () => this.applyVideoRange();
    if (stage === "extract") return this.video.processedFramesOrigin === "source"
      ? async () => this.activateStep(TASK_STEPS.video[3])
      : async () => {
          await this.video.handleExtract();
          if (this.video.processedFramesOrigin === "source") await this.activateStep(TASK_STEPS.video[3]);
        };
    if (stage === "redraw") {
      if (this.video.processedFramesOrigin === "redraw") return async () => this.activateStep(TASK_STEPS.video[4]);
      if (this.video.redrawRun) return async () => this.video.handleContinueRedraw();
      return async () => this.video.handleStartRedraw();
    }
    return async () => { await this.video.handleSave(); };
  }

  private buildSecondaryAction(): (() => Promise<void>) | null {
    if (!this.route) return null;
    if (this.route.kind === "image" && this.route.stage === "matting") {
      return async () => this.activateStep(TASK_STEPS.image[2]);
    }
    if ((this.route.kind === "image" || this.route.kind === "sprite") && this.route.stage === "bounds") {
      const kind = this.route.kind;
      return async () => this.advanceSpriteToPreview(kind);
    }
    if (this.route.kind === "video" && this.route.stage === "redraw") {
      return async () => this.activateStep(TASK_STEPS.video[4]);
    }
    return null;
  }

  private isStepUnlocked(stage: string): boolean {
    return this.route
      ? isTaskStepUnlocked(this.route, stage, this.presentationContext())
      : false;
  }

  private hasDownstreamResults(): boolean {
    if (!this.route) return false;
    if (this.route.kind === "video") return this.video.processedFrames.length > 0 || Boolean(this.video.redrawRun);
    return this.generator.mattingDirty || this.sprite.frameController.hasFrames();
  }

  private async applyVideoRange(): Promise<void> {
    if (this.video.processedFrames.length > 0 || this.video.redrawRun) {
      if (!window.confirm("应用新的选段或裁切会清除已抽取帧、重绘运行和预览结果。继续吗？")) return;
      if (this.video.redrawRun) await this.video.handleDiscardRedraw();
      this.video.clearGeneratedOutput();
    }
    await this.activateStep(TASK_STEPS.video[2]);
  }

  private async advanceSpriteToPreview(kind: "image" | "sprite"): Promise<void> {
    if (!this.sprite.frameController.hasFrames()) await this.sprite.handleLoadSplit();
    if (!this.sprite.frameController.hasFrames()) throw new Error("未能生成预览帧，请检查网格、区域和边界设置");
    const steps = TASK_STEPS[kind];
    this.setRoute({ kind, stage: steps[steps.length - 1].id } as TaskRoute);
  }

  private async resetWorkspaceAndReload(beforeReload?: () => void): Promise<void> {
    const shouldResume = await this.beforeWorkspaceReset?.() ?? false;
    try {
      await resetWorkspace();
      beforeReload?.();
      window.location.reload();
    } catch (error) {
      if (shouldResume) this.resumeWorkspace?.();
      window.alert(`重置工作区失败：${getErrorMessage(error)}`);
    }
  }

  private isBusy(): boolean {
    return this.generator.workflowState === "generating"
      || this.generator.workflowState === "optimizing"
      || this.generator.workflowState === "mattingProcessing"
      || this.video.isBusy();
  }

  private surfaceId(): string {
    return this.route ? getTaskSurfaceId(this.route) : "page-generator";
  }

  private presentationContext(): TaskPresentationContext {
    return {
      hasImageSelection: Boolean(this.generator.selectedGeneratedPath),
      mattingDirty: this.generator.mattingDirty,
      hasSpriteImage: Boolean(this.sprite.sheetImage),
      hasSpriteFrames: this.sprite.frameController.hasFrames(),
      spritePath: this.sprite.sheetImagePath,
      videoSourceMode: this.videoSourceMode,
      hasVideo: Boolean(this.video.sourcePath && this.video.videoMeta),
      videoName: this.video.sourceName,
      videoOutputOrigin: this.video.processedFramesOrigin,
      hasVideoOutput: this.video.processedFrames.length > 0,
      hasRedrawRun: Boolean(this.video.redrawRun),
    };
  }

  private stepIndex(stage: string): number {
    return this.route ? TASK_STEPS[this.route.kind].findIndex((item) => item.id === stage) : -1;
  }

  private notifyRouteChanged(): void {
    document.dispatchEvent(new CustomEvent(TASK_ROUTE_CHANGE_EVENT));
    window.requestAnimationFrame(() => this.els.stageTitle.focus());
  }
}

export function requireTaskCoordinator(): TaskCoordinator {
  if (!currentCoordinator) throw new Error("任务工作台尚未初始化");
  return currentCoordinator;
}

export function advanceImageTaskToGrid(): Promise<void> {
  return requireTaskCoordinator().advanceImageToGrid();
}

export function startImageTaskWithReference(path: string, name: string): Promise<void> {
  return requireTaskCoordinator().requestImageTaskWithReference(path, name);
}
