import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outDir = path.join(tmpdir(), "spriteanime-bounds-tests");
const require = createRequire(import.meta.url);

function compileCommonJs(sourceRelativePath, outputName) {
  const sourcePath = path.join(root, sourceRelativePath);
  const compiledPath = path.join(outDir, outputName);
  const transpiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: sourcePath,
  });
  writeFileSync(compiledPath, transpiled.outputText);
  return compiledPath;
}

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
compileCommonJs("src/pages/sprite/utils.ts", "utils.js");
const detectionPath = compileCommonJs("src/pages/sprite/bounds-detection.ts", "bounds-detection.cjs");
const { detectExpandedFrameBounds } = require(detectionPath);

function makeImage(width, height, rgba = [255, 255, 255, 255]) {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let index = 0; index < width * height; index += 1) {
    data.set(rgba, index * 4);
  }
  return data;
}

function fillRect(data, imageWidth, x, y, width, height, rgba = [0, 0, 0, 255]) {
  for (let py = y; py < y + height; py += 1) {
    for (let px = x; px < x + width; px += 1) {
      data.set(rgba, (py * imageWidth + px) * 4);
    }
  }
}

function detect(data, cellRects, expandPixels = 5) {
  return detectExpandedFrameBounds(
    data,
    24,
    12,
    cellRects,
    { r: 255, g: 255, b: 255, a: 255 },
    20,
    expandPixels
  );
}

function testExpansionCanCrossGridLineWhenConnectedToFrameSeed() {
  const data = makeImage(24, 12);
  const cellRects = [
    { x: 0, y: 0, width: 10, height: 12 },
    { x: 10, y: 0, width: 10, height: 12 },
  ];
  fillRect(data, 24, 6, 4, 7, 3);
  fillRect(data, 24, 16, 4, 3, 3);

  const bounds = detect(data, cellRects);

  assert.deepEqual(bounds[0], { x: 6, y: 4, width: 7, height: 3 });
  assert.deepEqual(bounds[1], { x: 16, y: 4, width: 3, height: 3 });
  assert.ok(bounds[0].x + bounds[0].width > cellRects[0].x + cellRects[0].width);
}

function testGridStillControlsWhichFrameReceivesOverflow() {
  const data = makeImage(24, 12);
  const cellRects = [
    { x: 0, y: 0, width: 8, height: 12 },
    { x: 8, y: 0, width: 12, height: 12 },
  ];
  fillRect(data, 24, 6, 4, 7, 3);

  const bounds = detect(data, cellRects);

  assert.equal(bounds[0], null);
  assert.deepEqual(bounds[1], { x: 6, y: 4, width: 7, height: 3 });
}

function testDisconnectedNeighborFrameIsNotSwallowedByExpansion() {
  const data = makeImage(24, 12);
  const cellRects = [
    { x: 0, y: 0, width: 10, height: 12 },
    { x: 10, y: 0, width: 10, height: 12 },
  ];
  fillRect(data, 24, 7, 4, 2, 3);
  fillRect(data, 24, 11, 4, 2, 3);

  const bounds = detect(data, cellRects);

  assert.deepEqual(bounds[0], { x: 7, y: 4, width: 2, height: 3 });
  assert.deepEqual(bounds[1], { x: 11, y: 4, width: 2, height: 3 });
}

testExpansionCanCrossGridLineWhenConnectedToFrameSeed();
testGridStillControlsWhichFrameReceivesOverflow();
testDisconnectedNeighborFrameIsNotSwallowedByExpansion();

rmSync(outDir, { recursive: true, force: true });
console.log("Bounds detection tests passed.");
