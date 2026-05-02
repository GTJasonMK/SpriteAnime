export interface SpriteElements {
  imageSelect: HTMLSelectElement;
  pickImage: HTMLButtonElement;
  rows: HTMLInputElement;
  cols: HTMLInputElement;
  gridPresets: HTMLButtonElement[];
  resetGridLines: HTMLButtonElement;
  regionX: HTMLInputElement;
  regionY: HTMLInputElement;
  regionW: HTMLInputElement;
  regionH: HTMLInputElement;
  regionFull: HTMLButtonElement;
  autoTrim: HTMLInputElement;
  autoExpand: HTMLInputElement;
  autoBgMode: HTMLSelectElement;
  autoTrimMode: HTMLSelectElement;
  autoThreshold: HTMLInputElement;
  detectBounds: HTMLButtonElement;
  autoExpandToggle: HTMLElement;
  boundaryAdd: HTMLButtonElement;
  boundaryEdit: HTMLButtonElement;
  boundaryDelete: HTMLButtonElement;
  boundaryPanel: HTMLElement;
  boundaryTitle: HTMLElement;
  boundaryLeft: HTMLInputElement;
  boundaryTop: HTMLInputElement;
  boundaryRight: HTMLInputElement;
  boundaryBottom: HTMLInputElement;
  boundaryAnchorX: HTMLInputElement;
  boundaryAnchorCenter: HTMLButtonElement;
  boundaryApply: HTMLButtonElement;
  boundaryCancel: HTMLButtonElement;
  boundaryClose: HTMLButtonElement;
  boundaryEditorDelete: HTMLButtonElement;
  spriteSidebar: HTMLElement;
  previewGrid: HTMLButtonElement;
  loadSplit: HTMLButtonElement;
  thumbnailList: HTMLElement;
  selectAll: HTMLButtonElement;
  invert: HTMLButtonElement;
  clear: HTMLButtonElement;
  canvas: HTMLCanvasElement;
  placeholder: HTMLElement;
  prevFrame: HTMLButtonElement;
  playPause: HTMLButtonElement;
  nextFrame: HTMLButtonElement;
  fpsSlider: HTMLInputElement;
  fpsLabel: HTMLSpanElement;
  scaleSlider: HTMLInputElement;
  scaleLabel: HTMLSpanElement;
  frameInfo: HTMLSpanElement;
  frameSizeInfo: HTMLSpanElement;
  export: HTMLButtonElement;
}

export function cacheSpriteElements(): SpriteElements {
  const g = (id: string) => document.getElementById(id) as HTMLElement;
  return {
    imageSelect: g("sprite-image-select") as HTMLSelectElement,
    pickImage: g("btn-pick-image") as HTMLButtonElement,
    rows: g("rows-input") as HTMLInputElement,
    cols: g("cols-input") as HTMLInputElement,
    gridPresets: Array.from(document.querySelectorAll<HTMLButtonElement>(".grid-preset")),
    resetGridLines: g("btn-reset-grid-lines") as HTMLButtonElement,
    regionX: g("region-x-input") as HTMLInputElement,
    regionY: g("region-y-input") as HTMLInputElement,
    regionW: g("region-w-input") as HTMLInputElement,
    regionH: g("region-h-input") as HTMLInputElement,
    regionFull: g("btn-region-full") as HTMLButtonElement,
    autoTrim: g("auto-trim-checkbox") as HTMLInputElement,
    autoExpand: g("auto-expand-checkbox") as HTMLInputElement,
    autoBgMode: g("auto-bg-mode-select") as HTMLSelectElement,
    autoTrimMode: g("auto-trim-mode-select") as HTMLSelectElement,
    autoThreshold: g("auto-trim-threshold-input") as HTMLInputElement,
    detectBounds: g("btn-detect-bounds") as HTMLButtonElement,
    autoExpandToggle: g("auto-expand-toggle"),
    boundaryAdd: g("btn-boundary-add") as HTMLButtonElement,
    boundaryEdit: g("btn-boundary-edit") as HTMLButtonElement,
    boundaryDelete: g("btn-boundary-delete") as HTMLButtonElement,
    boundaryPanel: g("boundary-editor-panel"),
    boundaryTitle: g("boundary-editor-title"),
    boundaryLeft: g("boundary-left-input") as HTMLInputElement,
    boundaryTop: g("boundary-top-input") as HTMLInputElement,
    boundaryRight: g("boundary-right-input") as HTMLInputElement,
    boundaryBottom: g("boundary-bottom-input") as HTMLInputElement,
    boundaryAnchorX: g("boundary-anchor-x-input") as HTMLInputElement,
    boundaryAnchorCenter: g("btn-boundary-anchor-center") as HTMLButtonElement,
    boundaryApply: g("btn-boundary-editor-apply") as HTMLButtonElement,
    boundaryCancel: g("btn-boundary-editor-cancel") as HTMLButtonElement,
    boundaryClose: g("btn-boundary-editor-close") as HTMLButtonElement,
    boundaryEditorDelete: g("btn-boundary-editor-delete") as HTMLButtonElement,
    spriteSidebar: document.querySelector<HTMLElement>(".sprite-sidebar")!,
    previewGrid: g("btn-preview-grid") as HTMLButtonElement,
    loadSplit: g("btn-load-split") as HTMLButtonElement,
    thumbnailList: g("thumbnail-list"),
    selectAll: g("btn-select-all") as HTMLButtonElement,
    invert: g("btn-invert") as HTMLButtonElement,
    clear: g("btn-clear-sel") as HTMLButtonElement,
    canvas: g("preview-canvas") as HTMLCanvasElement,
    placeholder: g("preview-placeholder"),
    prevFrame: g("btn-prev-frame") as HTMLButtonElement,
    playPause: g("btn-play-pause") as HTMLButtonElement,
    nextFrame: g("btn-next-frame") as HTMLButtonElement,
    fpsSlider: g("fps-slider") as HTMLInputElement,
    fpsLabel: g("fps-label"),
    scaleSlider: g("scale-slider") as HTMLInputElement,
    scaleLabel: g("scale-label"),
    frameInfo: g("frame-info"),
    frameSizeInfo: g("frame-size-info"),
    export: g("btn-export") as HTMLButtonElement,
  };
}
