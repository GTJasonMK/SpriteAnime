import { convertFileSrc } from "@tauri-apps/api/core";

/// 帧缩略图组件
export class FrameThumbnail {
  private container: HTMLDivElement;
  private img: HTMLImageElement;
  private orderLabel: HTMLSpanElement;
  private _index: number;

  constructor(
    index: number,
    source: { path: string },
    onClick: (index: number) => void
  ) {
    this._index = index;

    this.container = document.createElement("div");
    this.container.className = "frame-thumb";
    this.container.title = `帧 #${index}`;

    this.img = document.createElement("img");
    this.img.crossOrigin = "anonymous";
    this.img.src = convertFileSrc(source.path);
    this.img.alt = `#${index}`;

    // 序号标签（右下角）
    const label = document.createElement("span");
    label.className = "frame-index";
    label.textContent = `#${index}`;

    // 选中顺序标签（左上角）
    this.orderLabel = document.createElement("span");
    this.orderLabel.className = "frame-order";
    this.orderLabel.style.display = "none";

    this.container.appendChild(this.img);
    this.container.appendChild(label);
    this.container.appendChild(this.orderLabel);

    this.container.addEventListener("click", () => {
      onClick(this._index);
    });
  }

  get index(): number {
    return this._index;
  }

  setSelected(v: boolean, order: number): void {
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
