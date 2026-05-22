export interface VideoFrameListItem {
  url: string;
  time: number;
}

export function renderVideoFrameList(options: {
  container: HTMLElement;
  frames: VideoFrameListItem[];
  currentIndex: number;
  formatTime: (seconds: number) => string;
  onSelect: (index: number) => void;
}): void {
  const { container, frames, currentIndex, formatTime, onSelect } = options;
  container.innerHTML = "";
  if (frames.length === 0) {
    container.innerHTML = '<div class="placeholder-text">选择视频后生成序列帧图</div>';
    return;
  }

  frames.forEach((frame, index) => {
    const item = document.createElement("div");
    item.className = "frame-thumb video-frame-thumb";
    item.classList.toggle("current", index === currentIndex);
    item.addEventListener("click", () => onSelect(index));

    const img = document.createElement("img");
    img.src = frame.url;
    img.alt = `抽取帧 ${index + 1}`;

    const label = document.createElement("span");
    label.className = "frame-index";
    label.textContent = `${index + 1} · ${formatTime(frame.time)}`;

    item.appendChild(img);
    item.appendChild(label);
    container.appendChild(item);
  });
}
