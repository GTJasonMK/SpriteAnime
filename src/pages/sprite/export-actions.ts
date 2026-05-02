import {
  exportFrames,
  exportGif,
  selectDirectory,
  type ExportFrame,
  type FrameData,
} from "../../api/commands";
import {
  getFileName,
  joinPath,
  sanitizePathSegment,
  stripFileExtension,
  stripGifExtension,
} from "./utils";

interface SpriteExportOptions {
  frames: FrameData[];
  selectedIndices: number[];
  sheetImagePath: string;
  fps: number;
}

type ExportMode = "folder" | "gif";

interface ExportDialogResult {
  mode: ExportMode;
  name: string;
}

export async function handleSpriteExport(options: SpriteExportOptions): Promise<void> {
  if (options.selectedIndices.length === 0) {
    alert("请先选择要导出的帧");
    return;
  }

  const exportOptions = await requestExportOptions({
    folder: getDefaultExportFolderName(options.sheetImagePath),
    gif: getDefaultExportGifName(options.sheetImagePath),
  });
  if (!exportOptions) {
    return;
  }

  if (exportOptions.mode === "gif") {
    await exportSelectedGif(options, exportOptions.name);
  } else {
    await exportSelectedFrameFolder(options, exportOptions.name);
  }
}

function requestExportOptions(defaultNames: Record<ExportMode, string>): Promise<ExportDialogResult | null> {
  return new Promise((resolve) => {
    let mode: ExportMode = "folder";
    const draftNames: Record<ExportMode, string> = {
      folder: defaultNames.folder,
      gif: defaultNames.gif,
    };

    const overlay = document.createElement("div");
    overlay.className = "modal-overlay export-dialog-overlay";
    overlay.setAttribute("role", "dialog");
    overlay.setAttribute("aria-modal", "true");
    overlay.innerHTML = `
      <div class="modal-content export-dialog">
        <div class="modal-header">
          <h2>导出动画</h2>
          <button class="btn-close-modal" type="button" data-action="close" aria-label="关闭">×</button>
        </div>
        <div class="modal-body">
          <div class="export-mode-grid">
            <button class="export-mode-option selected" type="button" data-mode="folder">
              <span class="export-mode-title">序列帧文件夹</span>
              <span class="export-mode-desc">创建文件夹并导出 PNG 帧</span>
            </button>
            <button class="export-mode-option" type="button" data-mode="gif">
              <span class="export-mode-title">GIF 动图</span>
              <span class="export-mode-desc">按当前 FPS 导出循环预览</span>
            </button>
          </div>
          <div class="form-group export-name-group">
            <label for="export-name-input" id="export-name-label">文件夹名称</label>
            <input type="text" id="export-name-input" autocomplete="off" />
          </div>
          <p class="export-dialog-note" id="export-dialog-note">
            将先选择父目录，然后创建同名文件夹，帧文件按“名称_0.png”连续命名。
          </p>
        </div>
        <div class="modal-footer export-dialog-footer">
          <button class="btn-sm" type="button" data-action="cancel">取消</button>
          <button class="btn-primary" type="button" data-action="submit">继续导出</button>
        </div>
      </div>
    `;

    const nameInput = overlay.querySelector<HTMLInputElement>("#export-name-input")!;
    const nameLabel = overlay.querySelector<HTMLElement>("#export-name-label")!;
    const note = overlay.querySelector<HTMLElement>("#export-dialog-note")!;
    const modeButtons = Array.from(overlay.querySelectorAll<HTMLButtonElement>("[data-mode]"));
    const closeButtons = Array.from(overlay.querySelectorAll<HTMLButtonElement>("[data-action='close'], [data-action='cancel']"));
    const submitButton = overlay.querySelector<HTMLButtonElement>("[data-action='submit']")!;

    const setMode = (nextMode: ExportMode): void => {
      if (nextMode !== mode) {
        draftNames[mode] = nameInput.value;
      }
      mode = nextMode;
      nameInput.value = draftNames[mode];
      nameLabel.textContent = mode === "folder" ? "文件夹名称" : "GIF 文件名";
      note.textContent = mode === "folder"
        ? "将先选择父目录，然后创建同名文件夹，帧文件按“名称_0.png”连续命名。"
        : "将选择导出目录，并使用当前播放 FPS 生成一个 GIF 文件。";
      modeButtons.forEach((button) => {
        button.classList.toggle("selected", button.dataset.mode === mode);
      });
      nameInput.focus();
      nameInput.select();
    };

    const finish = (result: ExportDialogResult | null): void => {
      document.removeEventListener("keydown", handleKeyDown);
      overlay.remove();
      resolve(result);
    };

    const submit = (): void => {
      const name = nameInput.value.trim();
      if (!name) {
        alert(mode === "folder" ? "文件夹名称不能为空" : "GIF 文件名不能为空");
        nameInput.focus();
        return;
      }
      finish({ mode, name });
    };

    function handleKeyDown(event: KeyboardEvent): void {
      if (event.key === "Escape") {
        finish(null);
      } else if (event.key === "Enter" && document.activeElement === nameInput) {
        event.preventDefault();
        submit();
      }
    }

    modeButtons.forEach((button) => {
      button.addEventListener("click", () => setMode(button.dataset.mode as ExportMode));
    });
    closeButtons.forEach((button) => {
      button.addEventListener("click", () => finish(null));
    });
    submitButton.addEventListener("click", submit);
    overlay.addEventListener("click", (event) => {
      if (event.target === overlay) {
        finish(null);
      }
    });
    document.addEventListener("keydown", handleKeyDown);

    document.body.appendChild(overlay);
    setMode("folder");
  });
}

async function exportSelectedFrameFolder(options: SpriteExportOptions, inputName: string): Promise<void> {
  const folderName = sanitizePathSegment(inputName);
  if (!folderName) {
    alert("文件夹名称不能为空");
    return;
  }

  try {
    const parentDir = await selectDirectory();
    if (!parentDir) return;
    const outputDir = joinPath(parentDir, folderName);
    const savedPaths = await exportFrames(getSelectedExportFrames(options), outputDir, folderName);
    alert(`成功导出 ${savedPaths.length} 个帧到 ${outputDir}`);
  } catch (err) {
    if (String(err) !== "用户取消选择") {
      console.error("[sprite] 导出失败:", err);
      alert("导出失败: " + String(err));
    }
  }
}

async function exportSelectedGif(options: SpriteExportOptions, inputName: string): Promise<void> {
  const fileName = sanitizePathSegment(stripGifExtension(inputName));
  if (!fileName) {
    alert("GIF 文件名不能为空");
    return;
  }

  try {
    const outputDir = await selectDirectory();
    if (!outputDir) return;
    const savedPath = await exportGif(
      getSelectedExportFrames(options),
      outputDir,
      fileName,
      options.fps
    );
    alert(`成功导出 GIF：${savedPath}`);
  } catch (err) {
    if (String(err) !== "用户取消选择") {
      console.error("[sprite] 导出GIF失败:", err);
      alert("导出GIF失败: " + String(err));
    }
  }
}

function getSelectedExportFrames(options: SpriteExportOptions): ExportFrame[] {
  return options.selectedIndices.map((idx) => ({
    index: idx,
    path: options.frames[idx].path,
    base64: options.frames[idx].base64,
    anchorX: options.frames[idx].anchorX,
  }));
}

function getDefaultExportFolderName(sheetImagePath: string): string {
  const sourceName = stripFileExtension(getFileName(sheetImagePath || ""));
  const fallback = sourceName ? `${sourceName}_frames` : "sprite_frames";
  return sanitizePathSegment(fallback) || "sprite_frames";
}

function getDefaultExportGifName(sheetImagePath: string): string {
  const sourceName = stripFileExtension(getFileName(sheetImagePath || ""));
  const fallback = sourceName ? `${sourceName}_animation` : "sprite_animation";
  return sanitizePathSegment(fallback) || "sprite_animation";
}
