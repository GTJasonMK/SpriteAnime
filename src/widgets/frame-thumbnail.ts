import { convertFileSrc } from "@tauri-apps/api/core";

/// 帧缩略图组件
export class FrameThumbnail {
  private container: HTMLDivElement;
  private img: HTMLImageElement;
  private label: HTMLSpanElement;
  private orderLabel: HTMLSpanElement;
  private _index: number;
  private _selected: boolean = false;
  private _current: boolean = false;
  private onClick: (index: number) => void;

  constructor(
    index: number,
    source: { path?: string; base64?: string },
    onClick: (index: number) => void
  ) {
    this._index = index;
    this.onClick = onClick;

    this.container = document.createElement("div");
    this.container.className = "frame-thumb";
    this.container.title = `帧 #${index}`;

    this.img = document.createElement("img");
    this.img.crossOrigin = "anonymous";
    this.img.src = source.path
      ? convertFileSrc(source.path)
      : `data:image/png;base64,${source.base64 || ""}`;
    this.img.alt = `#${index}`;

    // 序号标签（右下角）
    this.label = document.createElement("span");
    this.label.className = "frame-index";
    this.label.textContent = `#${index}`;

    // 选中顺序标签（左上角）
    this.orderLabel = document.createElement("span");
    this.orderLabel.className = "frame-order";
    this.orderLabel.style.display = "none";

    this.container.appendChild(this.img);
    this.container.appendChild(this.label);
    this.container.appendChild(this.orderLabel);

    this.container.addEventListener("click", () => {
      this.onClick(this._index);
    });
  }

  get index(): number {
    return this._index;
  }

  get selected(): boolean {
    return this._selected;
  }

  get current(): boolean {
    return this._current;
  }

  setSelected(v: boolean, order: number = -1): void {
    this._selected = v;
    if (v) {
      this.container.classList.add("selected");
      if (order >= 0) {
        this.orderLabel.textContent = `${order + 1}`;
        this.orderLabel.style.display = "block";
      }
    } else {
      this.container.classList.remove("selected");
      this.orderLabel.style.display = "none";
    }
  }

  setCurrent(v: boolean): void {
    this._current = v;
    this.container.classList.toggle("current", v);
    this.container.setAttribute("aria-current", v ? "true" : "false");
  }

  getElement(): HTMLElement {
    return this.container;
  }

  dispose(): void {
    this.img.src = "";
    this.container.remove();
  }
}
