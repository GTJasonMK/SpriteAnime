import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { fillRect, makeImage } from "./pixel-test-helpers.mjs";
import {
  cleanupTempDir,
  compileCommonJsModule,
  getRepoRoot,
  resetTempDir,
  runTests,
} from "./ts-test-helpers.mjs";

const root = getRepoRoot(import.meta.url);
const outDir = resetTempDir("spriteanime-bounds-tests");
const require = createRequire(import.meta.url);

compileCommonJsModule(root, outDir, "src/utils/number.ts");
compileCommonJsModule(root, outDir, "src/utils/path.ts");
compileCommonJsModule(root, outDir, "src/pages/sprite/utils.ts");
const detectionPath = compileCommonJsModule(root, outDir, "src/pages/sprite/bounds-detection.ts");
const { detectExpandedFrameBounds } = require(detectionPath);

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

runTests([
  testExpansionCanCrossGridLineWhenConnectedToFrameSeed,
  testGridStillControlsWhichFrameReceivesOverflow,
  testDisconnectedNeighborFrameIsNotSwallowedByExpansion,
]);

cleanupTempDir(outDir);
console.log("Bounds detection tests passed.");
