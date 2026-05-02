import type { SpriteElements } from "./dom";
import { getFileName } from "./utils";

interface GeneratedImageProvider {
  getLastGeneratedImages(): string[];
  getSelectedGeneratedImagePath(): string | null;
}

interface SyncGeneratedImageSourcesOptions {
  els: SpriteElements;
  generatorPage: GeneratedImageProvider;
  locked: boolean;
  selectLatest?: boolean;
  applyPreferredGrid?: boolean;
  onApplyPreferredGrid: () => void;
}

export function addImageSource(
  els: SpriteElements,
  path: string,
  select: boolean = true
): boolean {
  const existing = Array.from(els.imageSelect.options).find((opt) => opt.value === path);
  if (existing) {
    if (select) {
      els.imageSelect.value = path;
    }
    return false;
  }

  const opt = document.createElement("option");
  opt.value = path;
  opt.textContent = getFileName(path);
  els.imageSelect.appendChild(opt);
  if (select) {
    els.imageSelect.value = path;
  }
  return true;
}

export function syncGeneratedImageSources(options: SyncGeneratedImageSourcesOptions): void {
  const {
    els,
    generatorPage,
    locked,
    selectLatest,
    applyPreferredGrid,
    onApplyPreferredGrid,
  } = options;
  const paths = generatorPage.getLastGeneratedImages();
  const selectedGeneratedPath = selectLatest && !locked
    ? generatorPage.getSelectedGeneratedImagePath()
    : null;

  paths.forEach((path, index) => {
    const shouldSelect = Boolean(
      selectLatest &&
        !locked &&
        (selectedGeneratedPath ? path === selectedGeneratedPath : index === paths.length - 1)
    );
    addImageSource(els, path, shouldSelect);
  });

  if (selectLatest && !locked && paths.length > 0) {
    els.imageSelect.value = selectedGeneratedPath || paths[paths.length - 1];
  }
  if (applyPreferredGrid && !locked) {
    onApplyPreferredGrid();
  }
}
