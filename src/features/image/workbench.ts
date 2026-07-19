import { convertFileSrc } from "@tauri-apps/api/core";
import {
  clearWorkbenchRecords as clearWorkbenchRecordsApi,
  deleteWorkbenchRecord,
  importImageToLibrary,
  openImageFile,
  readWorkbenchRecords,
  upsertWorkbenchRecords,
} from "../../api/commands";
import { isUserCancelError } from "../../utils/errors";
import type { GeneratorPage } from "./image-page";
import { recordToWorkbenchDto, requiredDisplayName, requiredFileNameStem } from "./helpers";
import type { GeneratedImageRecord } from "./types";

export const generatorWorkbenchMethods = {
  async loadWorkbenchRecords(): Promise<void> {
    try {
      const records = await readWorkbenchRecords(200);
      const selectedPath = records.length === 0 ? null : records[records.length - 1].path;
      this.applyWorkbenchDtos(records, selectedPath);
      if (this.generatedRecords.length === 0) return;
      this.els.workspaceEmpty.style.display = "none";
      this.els.resultCard.style.display = "flex";
      this.els.resultActions.style.display = "flex";
      this.renderGallery();
      this.syncWorkflowControls();
    } catch (error) {
      this.setToolbarError("工作台记录加载失败", error);
    }
  },

  async handlePickReferenceImage(): Promise<void> {
    if (!this.canRunGeneratorAction("editGenerationParams")) return;
    try {
      const file = await openImageFile();
      const imported = await importImageToLibrary(file.file_path);
      this.setReferenceImage(imported.file_path, imported.file_name);
      this.els.toolbarStatus.textContent = "已选择参考图";
    } catch (error) {
      if (!isUserCancelError(error)) this.setToolbarError("选择参考图失败", error);
    }
  },

  setReferenceImage(path: string, name: string): void {
    this.referenceImagePath = path;
    this.referenceImageName = requiredDisplayName(name, path, "参考图");
    this.els.referenceImageName.value = this.referenceImageName;
    this.els.referenceImagePreview.src = convertFileSrc(path);
    this.els.referenceImagePreview.style.display = "block";
    this.els.referenceImageEmpty.style.display = "none";
    this.els.clearReferenceImage.disabled = false;
  },

  clearReferenceImage(): void {
    this.referenceImagePath = "";
    this.referenceImageName = "";
    this.els.referenceImagePreview.removeAttribute("src");
    this.els.referenceImagePreview.style.display = "none";
    this.els.referenceImageEmpty.style.display = "inline";
    this.els.referenceImageName.value = "无参考图";
    this.els.clearReferenceImage.disabled = true;
    this.els.toolbarStatus.textContent = "已移除参考图";
  },

  async handleAddRecord(): Promise<void> {
    if (!this.canRunGeneratorAction("addRecord")) return;
    try {
      const file = await openImageFile();
      const imported = await importImageToLibrary(file.file_path);
      const now = new Date();
      const record: GeneratedImageRecord = {
        id: `manual-${Date.now()}`,
        path: imported.file_path,
        label: requiredFileNameStem(imported.file_name, imported.file_path, "手动添加图片"),
        prompt: "",
        model: "手动添加",
        createdAt: now,
        updatedAt: now,
      };
      const records = await upsertWorkbenchRecords([recordToWorkbenchDto(record)]);
      this.applyWorkbenchDtos(records, record.path);
      this.renderGallery();
      this.els.toolbarStatus.textContent = "已添加记录";
    } catch (error) {
      if (!isUserCancelError(error)) this.setToolbarError("添加记录失败", error);
    }
  },

  async deleteSelectedRecord(): Promise<void> {
    if (!this.canRunGeneratorAction("deleteRecord")) return;
    const record = this.getSelectedRecord();
    if (!record || !window.confirm("仅从工作台移除此记录，不会删除图片文件。继续？")) return;
    try {
      const currentIndex = this.generatedRecords.findIndex((item) => item.id === record.id);
      const records = await deleteWorkbenchRecord(record.id);
      const nextIndex = Math.min(Math.max(currentIndex, 0), records.length - 1);
      this.applyWorkbenchDtos(records, records.length === 0 ? null : records[nextIndex].path);
      this.renderGallery();
      this.els.toolbarStatus.textContent = "已移除记录";
    } catch (error) {
      this.setToolbarError("删除记录失败", error);
    }
  },

  async clearWorkbenchRecords(): Promise<void> {
    if (!this.canRunGeneratorAction("clearRecords") || !window.confirm("清空工作台记录？图片文件会保留在磁盘上。")) return;
    try {
      await clearWorkbenchRecordsApi();
      this.generatedRecords = [];
      this.selectedGeneratedPath = null;
      this.renderGallery();
      this.els.toolbarStatus.textContent = "记录已清空";
    } catch (error) {
      this.setToolbarError("清空记录失败", error);
    }
  },
} satisfies ThisType<GeneratorPage>;

export type GeneratorWorkbenchMethods = typeof generatorWorkbenchMethods;
