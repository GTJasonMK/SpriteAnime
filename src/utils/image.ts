import { convertFileSrc } from "@tauri-apps/api/core";

export function loadHtmlImageFromPath(path: string): Promise<HTMLImageElement> {
  return loadImageElement(convertFileSrc(path));
}

export function loadImageFromDataUrl(dataUrl: string): Promise<HTMLImageElement> {
  return loadImageElement(dataUrl);
}

function loadImageElement(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("图片加载失败"));
    img.src = src;
  });
}
