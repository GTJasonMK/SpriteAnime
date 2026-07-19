import { openImageFilePath, revealInExplorer } from "../../api/commands";
import {
  usesBackgroundDescription,
  type GenerationBackgroundMode,
} from "../../generation/constraints";
import { getDirectoryName } from "../../utils/path";
import type { GeneratorPage } from "./image-page";

export const generatorSettingsMethods = {
  bindEvents(): void {
    this.els.imageConstraintsBackground.addEventListener("change", () => this.syncImageConstraintControls());
    this.els.imageConstraintsEnabled.addEventListener("change", () => this.syncImageConstraintControls());
    [this.els.imageConstraintsRows, this.els.imageConstraintsCols].forEach((input) => {
      input.addEventListener("change", () => {
        const rows = Number(this.els.imageConstraintsRows.value);
        const cols = Number(this.els.imageConstraintsCols.value);
        if (Number.isInteger(rows) && rows > 0 && Number.isInteger(cols) && cols > 0) {
          this.preferredSpriteGrid = { rows, cols };
        }
      });
    });
    this.els.optimizePrompt.addEventListener("click", () => void this.handleOptimizePrompt());
    this.els.prompt.addEventListener("keydown", (event) => this.handlePromptHistoryKey(event));
    this.els.pickReferenceImage.addEventListener("click", () => void this.handlePickReferenceImage());
    this.els.clearReferenceImage.addEventListener("click", () => this.clearReferenceImage());
    this.els.viewImage.addEventListener("click", () => {
      if (!this.canRunGeneratorAction("openSelected") || !this.selectedGeneratedPath) return;
      void openImageFilePath(this.selectedGeneratedPath).catch((error) => this.setToolbarError("打开图片失败", error));
    });
    this.els.openDir.addEventListener("click", () => {
      if (!this.canRunGeneratorAction("revealSelected") || !this.selectedGeneratedPath) return;
      void revealInExplorer(getDirectoryName(this.selectedGeneratedPath)).catch((error) => this.setToolbarError("打开目录失败", error));
    });
    this.els.exitMatting.addEventListener("click", () => this.exitMattingMode());
    this.els.runMatting.addEventListener("click", () => void this.handleMakeTransparentBackground());
    this.els.undoMatting.addEventListener("click", () => this.undoMattingErase());
    this.els.redoMatting.addEventListener("click", () => this.redoMattingErase());
    this.els.saveMatting.addEventListener("click", () => void this.handleSaveMattingEdits());
    document.addEventListener("keydown", (event) => this.handleMattingKeyboardShortcuts(event));
    this.els.mattingCanvas.addEventListener("click", (event) => void this.handleMattingCanvasClick(event));
    [this.els.mattingTolerance, this.els.mattingFeather, this.els.mattingClickTolerance, this.els.mattingClickRadius]
      .forEach((input) => input.addEventListener("input", () => this.syncMattingLabels()));
    this.els.addRecord.addEventListener("click", () => void this.handleAddRecord());
    this.els.addRecordEmpty.addEventListener("click", () => void this.handleAddRecord());
    this.els.deleteRecord.addEventListener("click", () => void this.deleteSelectedRecord());
    this.els.clearRecords.addEventListener("click", () => void this.clearWorkbenchRecords());
    this.syncMattingLabels();
  },

  handlePromptHistoryKey(event: KeyboardEvent): void {
    if ((!event.ctrlKey && !event.metaKey) || this.promptHistory.length === 0) return;
    if (event.key === "ArrowUp" && this.historyIndex > 0) {
      event.preventDefault();
      this.historyIndex -= 1;
      this.els.prompt.value = this.promptHistory[this.historyIndex];
    } else if (event.key === "ArrowDown" && this.historyIndex < this.promptHistory.length) {
      event.preventDefault();
      this.historyIndex += 1;
      this.els.prompt.value = this.historyIndex < this.promptHistory.length
        ? this.promptHistory[this.historyIndex]
        : "";
    }
  },

  syncImageConstraintControls(): void {
    const locked = !this.canRunGeneratorAction("editGenerationParams");
    const enabled = this.els.imageConstraintsEnabled.checked;
    this.els.imageConstraintsEnabled.disabled = locked;
    this.els.imageConstraintsRows.disabled = locked || !enabled;
    this.els.imageConstraintsCols.disabled = locked || !enabled;
    this.els.imageConstraintsBackground.disabled = locked || !enabled;
    this.els.imageConstraintsFraming.disabled = locked || !enabled;
    this.els.imageConstraintsBackgroundDescription.disabled =
      locked || !enabled || !usesBackgroundDescription(
        this.els.imageConstraintsBackground.value as GenerationBackgroundMode
      );
  },
} satisfies ThisType<GeneratorPage>;

export type GeneratorSettingsMethods = typeof generatorSettingsMethods;
