import { convertFileSrc } from "@tauri-apps/api/core";
import { queryAll } from "../../utils/dom";

export interface GeneratedGalleryRecord {
  path: string;
  label: string;
  model: string;
  durationSeconds?: number;
  createdAt: Date;
}

export function renderGeneratedGallery(options: {
  container: HTMLElement;
  records: GeneratedGalleryRecord[];
  selectedPath: string | null;
  formatTime: (date: Date) => string;
  formatDuration: (value: number | undefined) => string;
  onSelect: (path: string) => void;
  onOpen: (path: string) => void;
  onImageLoadError: (path: string, role: "thumbnail" | "preview") => void;
}): void {
  const {
    container,
    records,
    selectedPath,
    formatTime,
    formatDuration,
    onSelect,
    onOpen,
    onImageLoadError,
  } = options;
  container.innerHTML = "";

  records.forEach((record, index) => {
    const item = document.createElement("div");
    item.className = "image-item";
    item.dataset.path = record.path;
    item.classList.toggle("selected", record.path === selectedPath);
    item.tabIndex = 0;

    const img = document.createElement("img");
    setImageSource(img, record.path, () => onImageLoadError(record.path, "thumbnail"));
    img.alt = `生成图片 ${index + 1}`;
    img.loading = "lazy";

    const meta = document.createElement("div");
    meta.className = "image-meta";
    const timeRow = document.createElement("span");
    timeRow.textContent = formatTime(record.createdAt);
    const durationRow = document.createElement("span");
    durationRow.textContent = `耗时 ${formatDuration(record.durationSeconds)}`;
    meta.appendChild(timeRow);
    meta.appendChild(durationRow);

    item.appendChild(img);
    item.appendChild(meta);
    item.addEventListener("click", () => onSelect(record.path));
    item.addEventListener("dblclick", () => onOpen(record.path));
    item.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        onSelect(record.path);
      }
    });
    container.appendChild(item);
  });
}

export function updateGeneratedGallerySelection(
  container: HTMLElement,
  selectedPath: string | null
): void {
  queryAll<HTMLElement>(".image-item", container).forEach((item) => {
    item.classList.toggle("selected", item.dataset.path === selectedPath);
  });
}

export function setSelectedGeneratedPreview(options: {
  image: HTMLImageElement;
  meta: HTMLElement;
  record: GeneratedGalleryRecord | null;
  formatTime: (date: Date) => string;
  formatDuration: (value: number | undefined) => string;
  onImageLoadError: (path: string, role: "thumbnail" | "preview") => void;
}): void {
  const { image, meta, record, formatTime, formatDuration, onImageLoadError } = options;
  if (!record) {
    meta.textContent = "未选择图片";
    image.removeAttribute("src");
    return;
  }

  setImageSource(image, record.path, () => onImageLoadError(record.path, "preview"));
  meta.textContent =
    `${record.label} · ${record.model} · ${formatTime(record.createdAt)} · 耗时 ${formatDuration(record.durationSeconds)}`;
}

function setImageSource(
  img: HTMLImageElement,
  path: string,
  onFailure: () => void
): void {
  img.src = convertFileSrc(path);
  img.onerror = onFailure;
}
