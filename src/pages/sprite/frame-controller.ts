import type { FrameData } from "../../api/commands";
import { CanvasPlayer } from "../../widgets/canvas-player";
import { FrameThumbnail } from "../../widgets/frame-thumbnail";
import type { SpriteElements } from "./dom";

interface FrameControllerHooks {
  renderFallbackPreview: () => boolean;
}

export class SpriteFrameController {
  private frames: FrameData[] = [];
  private selectedIndices: number[] = [];
  private currentFrame: number = 0;
  private fps: number = 24;
  private scale: number = 1.0;
  private isPlaying: boolean = false;
  private animFrameId: number | null = null;
  private thumbnails: FrameThumbnail[] = [];
  private currentThumbnailIndex: number | null = null;

  constructor(
    private els: SpriteElements,
    private canvasPlayer: CanvasPlayer,
    private hooks: FrameControllerHooks
  ) {}

  bindEvents(): void {
    this.els.selectAll.addEventListener("click", () => this.selectAll());
    this.els.invert.addEventListener("click", () => this.invertSelection());
    this.els.clear.addEventListener("click", () => this.clearSelection());

    this.els.playPause.addEventListener("click", () => this.togglePlay());
    this.els.prevFrame.addEventListener("click", () => this.stepFrame(-1));
    this.els.nextFrame.addEventListener("click", () => this.stepFrame(1));

    this.els.fpsSlider.addEventListener("input", () => {
      this.setFps(Number.parseInt(this.els.fpsSlider.value, 10));
    });

    this.els.scaleSlider.addEventListener("input", () => {
      this.setScale(Number.parseFloat(this.els.scaleSlider.value));
    });
  }

  getFrames(): FrameData[] {
    return this.frames;
  }

  getSelectedIndices(): number[] {
    return this.selectedIndices;
  }

  getFps(): number {
    return this.fps;
  }

  hasFrames(): boolean {
    return this.frames.length > 0;
  }

  stopPlayback(): void {
    this.isPlaying = false;
    this.els.playPause.textContent = "播放";
    this.els.playPause.classList.remove("btn-primary");

    if (this.animFrameId !== null) {
      cancelAnimationFrame(this.animFrameId);
      this.animFrameId = null;
    }
  }

  destroyLoadedFrames(): void {
    this.canvasPlayer.destroy();
  }

  clearCanvas(): void {
    this.canvasPlayer.clear();
  }

  clearFrameList(message: string): void {
    this.thumbnails.forEach((thumb) => thumb.dispose());
    this.thumbnails = [];
    this.frames = [];
    this.selectedIndices = [];
    this.currentFrame = 0;
    this.currentThumbnailIndex = null;

    this.els.thumbnailList.innerHTML = "";
    const placeholder = document.createElement("div");
    placeholder.className = "placeholder-text";
    placeholder.textContent = message;
    this.els.thumbnailList.appendChild(placeholder);

    this.els.export.disabled = true;
    this.updateFrameInfo();
  }

  async applyFrames(
    frames: FrameData[],
    rows: number,
    cols: number,
    sourceInfo: string
  ): Promise<boolean> {
    this.frames = frames;

    if (this.frames.length === 0) {
      this.clearFrameList("分割失败");
      return false;
    }

    await this.canvasPlayer.loadFrames(this.frames);
    this.buildThumbnailList();
    this.currentFrame = 0;
    this.selectAll();

    const frame = this.frames[0];
    const anchorNote = this.frames.some((item) => Number.isFinite(item.anchorX))
      ? " | 定位针对齐"
      : "";
    this.els.frameSizeInfo.textContent =
      `已拆分: ${rows}行 x ${cols}列 | 单帧: ${frame.width}x${frame.height} | ${sourceInfo}${anchorNote}`;
    this.els.export.disabled = false;
    this.els.placeholder.style.display = "none";
    this.els.canvas.closest(".preview-area")?.classList.add("has-image");
    return true;
  }

  renderCurrentFrame(): void {
    if (this.frames.length === 0) {
      this.updateCurrentThumbnail(null);
      if (!this.hooks.renderFallbackPreview()) {
        this.canvasPlayer.clear();
        this.els.placeholder.style.display = "block";
        this.els.canvas.closest(".preview-area")?.classList.remove("has-image");
      }
      this.updateFrameInfo();
      return;
    }

    if (this.selectedIndices.length === 0) {
      this.updateCurrentThumbnail(null);
      this.canvasPlayer.clear();
      this.els.placeholder.textContent = "未选择帧";
      this.els.placeholder.style.display = "block";
      this.els.canvas.closest(".preview-area")?.classList.remove("has-image");
      this.updateFrameInfo();
      return;
    }

    this.clampCurrentFrame();
    this.els.placeholder.style.display = "none";
    this.els.canvas.closest(".preview-area")?.classList.add("has-image");

    const frameIdx = this.selectedIndices[this.currentFrame];
    if (frameIdx >= 0 && frameIdx < this.frames.length) {
      this.canvasPlayer.renderFrame(frameIdx);
      this.updateCurrentThumbnail(frameIdx);
    } else {
      this.updateCurrentThumbnail(null);
    }

    this.updateFrameInfo();
  }

  updateFrameInfo(): void {
    const total = this.selectedIndices.length;
    this.els.frameInfo.textContent = `帧: ${total === 0 ? 0 : this.currentFrame + 1}/${total}`;
  }

  private buildThumbnailList(): void {
    this.thumbnails.forEach((thumb) => thumb.dispose());
    this.thumbnails = [];
    this.currentThumbnailIndex = null;

    this.els.thumbnailList.innerHTML = "";

    this.frames.forEach((frame, index) => {
      const thumb = new FrameThumbnail(index, frame, (idx) => {
        this.toggleFrameSelection(idx);
      });
      this.thumbnails.push(thumb);
      this.els.thumbnailList.appendChild(thumb.getElement());
    });
  }

  private toggleFrameSelection(index: number): void {
    const pos = this.selectedIndices.indexOf(index);
    if (pos >= 0) {
      this.selectedIndices.splice(pos, 1);
    } else {
      this.selectedIndices.push(index);
    }
    this.updateThumbnailStates();
  }

  private selectAll(): void {
    this.selectedIndices = this.frames.map((_, index) => index);
    this.updateThumbnailStates();
  }

  private invertSelection(): void {
    const newSelection: number[] = [];
    const set = new Set(this.selectedIndices);
    this.frames.forEach((_, index) => {
      if (!set.has(index)) newSelection.push(index);
    });
    this.selectedIndices = newSelection;
    this.updateThumbnailStates();
  }

  private clearSelection(): void {
    this.selectedIndices = [];
    this.updateThumbnailStates();
  }

  private updateThumbnailStates(): void {
    this.clampCurrentFrame();
    if (!this.isPlaying) {
      this.focusLastSelectedFrame();
    }

    const set = new Set(this.selectedIndices);
    this.thumbnails.forEach((thumb) => {
      const index = thumb.index;
      const order = this.selectedIndices.indexOf(index);
      thumb.setSelected(set.has(index), order);
    });

    if (this.selectedIndices.length === 0) {
      this.stopPlayback();
    }
    this.renderCurrentFrame();
  }

  private focusLastSelectedFrame(): void {
    if (this.selectedIndices.length === 0) {
      this.currentFrame = 0;
      return;
    }

    this.currentFrame = this.selectedIndices.length - 1;
  }

  private clampCurrentFrame(): void {
    if (this.selectedIndices.length === 0) {
      this.currentFrame = 0;
      return;
    }

    if (this.currentFrame < 0) {
      this.currentFrame = 0;
    } else if (this.currentFrame >= this.selectedIndices.length) {
      this.currentFrame = this.selectedIndices.length - 1;
    }
  }

  private updateCurrentThumbnail(frameIndex: number | null): void {
    if (this.currentThumbnailIndex === frameIndex) return;

    if (this.currentThumbnailIndex !== null) {
      this.thumbnails[this.currentThumbnailIndex]?.setCurrent(false);
    }

    this.currentThumbnailIndex = frameIndex;

    if (frameIndex !== null) {
      this.thumbnails[frameIndex]?.setCurrent(true);
    }
  }

  private togglePlay(): void {
    if (this.isPlaying) {
      this.stopPlayback();
    } else {
      if (this.selectedIndices.length === 0) {
        alert("请先选择至少一帧");
        return;
      }
      this.startPlayback();
    }
  }

  private startPlayback(): void {
    this.isPlaying = true;
    this.els.playPause.textContent = "暂停";
    this.els.playPause.classList.add("btn-primary");

    let lastTime = performance.now();

    const tick = (now: number) => {
      if (!this.isPlaying) return;

      const frameInterval = 1000 / Math.max(1, this.fps);
      if (now - lastTime >= frameInterval) {
        this.advanceFrame(1);
        lastTime = now;
      }

      this.animFrameId = requestAnimationFrame(tick);
    };

    this.animFrameId = requestAnimationFrame(tick);
  }

  private advanceFrame(delta: number): void {
    if (this.selectedIndices.length === 0) {
      this.renderCurrentFrame();
      return;
    }
    this.currentFrame =
      (this.currentFrame + delta + this.selectedIndices.length) %
      this.selectedIndices.length;
    this.renderCurrentFrame();
  }

  private stepFrame(delta: number): void {
    if (this.isPlaying) this.stopPlayback();
    this.advanceFrame(delta);
  }

  private setFps(fps: number): void {
    this.fps = fps;
    this.els.fpsLabel.textContent = String(this.fps);
  }

  private setScale(scale: number): void {
    this.scale = scale;
    this.els.scaleLabel.textContent = `${this.scale.toFixed(1)}x`;
    this.canvasPlayer.setScale(this.scale);
    if (!this.isPlaying) {
      this.renderCurrentFrame();
    }
  }
}
