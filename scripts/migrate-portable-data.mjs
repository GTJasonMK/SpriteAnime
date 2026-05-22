#!/usr/bin/env node
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(SCRIPT_DIR, "..");
const DEFAULT_TARGET_ROOT = path.join(
  PROJECT_ROOT,
  "src-tauri",
  "target",
  "debug",
  "SpriteAnimteData",
);
const DEFAULT_OLD_ASSET_ROOT = path.join(os.homedir(), "Pictures", "SpriteAnimte");
const DEFAULT_OLD_RUNTIME_ROOT = path.join(os.homedir(), ".local", "share", "sprite-animte");
const ASSET_DIR_NAME = "assets";
const ASSET_CATEGORIES = [
  "generated-images",
  "imported-images",
  "matted-images",
  "original-videos",
  "generated-videos",
  "video-sprite-sheets",
  "exported-frame-sets",
  "exported-gifs",
];
const TEMP_DIRS = ["temp_frames", "temp_videos", "temp_video_frames"];
const ACTIVE_RUNTIME_FILES = new Set(["config.json", "workbench_records.json"]);

const args = parseArgs(process.argv.slice(2));
const apply = args.flags.has("apply");
const targetRoot = path.resolve(args.values.target ?? DEFAULT_TARGET_ROOT);
const targetAssetRoot = path.join(targetRoot, ASSET_DIR_NAME);
const oldAssetRoot = path.resolve(args.values["old-assets"] ?? DEFAULT_OLD_ASSET_ROOT);
const oldRuntimeRoot = path.resolve(args.values["old-runtime"] ?? DEFAULT_OLD_RUNTIME_ROOT);
const timestamp = timestampForFile();
const pathMap = new Map();
const stats = {
  copied: 0,
  skippedIdentical: 0,
  conflictsRenamed: 0,
  jsonWritten: 0,
  backups: 0,
  missingSources: 0,
};

main().catch((error) => {
  console.error(error?.stack ?? String(error));
  process.exitCode = 1;
});

async function main() {
  if (args.flags.has("help")) {
    printHelp();
    return;
  }

  logHeader();
  await copyAssetCategories();
  await copyRuntimeTempDirs();
  await copyLogs();
  await copyRuntimeArchiveFiles();
  await migrateConfig();
  await migrateWorkbenchRecords();
  await ensureStandardDirs();
  await printVerification();
}

function parseArgs(argv) {
  const flags = new Set();
  const values = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) {
      throw new Error(`未知参数: ${arg}`);
    }
    const key = arg.slice(2);
    if (key === "apply" || key === "help") {
      flags.add(key);
      continue;
    }
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`参数 --${key} 需要值`);
    }
    values[key] = value;
    index += 1;
  }
  return { flags, values };
}

function printHelp() {
  console.log(`Usage: node scripts/migrate-portable-data.mjs [--apply] [--target DIR]

Options:
  --apply             Execute the migration. Without this, only print a dry run.
  --target DIR        Portable data root. Defaults to current dev executable data dir.
  --old-assets DIR    Old asset library root. Defaults to ~/Pictures/SpriteAnimte.
  --old-runtime DIR   Old runtime data root. Defaults to ~/.local/share/sprite-animte.
`);
}

function logHeader() {
  console.log(apply ? "[migrate] APPLY mode" : "[migrate] DRY-RUN mode");
  console.log(`[migrate] target root: ${targetRoot}`);
  console.log(`[migrate] target assets: ${targetAssetRoot}`);
  console.log(`[migrate] old assets: ${oldAssetRoot}`);
  console.log(`[migrate] old runtime: ${oldRuntimeRoot}`);
}

async function copyAssetCategories() {
  for (const category of ASSET_CATEGORIES) {
    await copyTree(
      path.join(oldAssetRoot, category),
      path.join(targetAssetRoot, category),
      { mapPaths: true },
    );
  }
}

async function copyRuntimeTempDirs() {
  for (const dirName of TEMP_DIRS) {
    await copyTree(
      path.join(oldRuntimeRoot, dirName),
      path.join(targetRoot, dirName),
      { mapPaths: true },
    );
  }
}

async function copyLogs() {
  await copyTree(path.join(oldRuntimeRoot, "logs"), path.join(targetRoot, "logs"));
  await copyTree(path.join(PROJECT_ROOT, "logs"), path.join(targetRoot, "logs"), {
    fileNamePrefix: "project-",
  });
}

async function copyRuntimeArchiveFiles() {
  if (!(await pathExists(oldRuntimeRoot))) {
    stats.missingSources += 1;
    console.log(`[migrate] source missing: ${oldRuntimeRoot}`);
    return;
  }

  const entries = await fs.readdir(oldRuntimeRoot, { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isFile() || ACTIVE_RUNTIME_FILES.has(entry.name)) {
      continue;
    }
    const source = path.join(oldRuntimeRoot, entry.name);
    const dest = path.join(targetRoot, "migration-archive", "old-runtime", entry.name);
    await copyFileUnique(source, dest);
  }

  const rootConfig = path.join(PROJECT_ROOT, "config.json");
  if (await pathExists(rootConfig)) {
    await copyFileUnique(
      rootConfig,
      path.join(targetRoot, "migration-archive", "project-config.json"),
    );
  }
}

async function migrateConfig() {
  const oldConfigPath = path.join(oldRuntimeRoot, "config.json");
  const targetConfigPath = path.join(targetRoot, "config.json");
  const projectConfigPath = path.join(PROJECT_ROOT, "config.json");

  const oldConfig = await readJsonIfExists(oldConfigPath);
  const targetConfig = await readJsonIfExists(targetConfigPath);
  const projectConfig = await readJsonIfExists(projectConfigPath);
  const config = cloneJson(oldConfig ?? targetConfig ?? projectConfig ?? {});

  config.save_dir = targetAssetRoot;
  config.prompt_history = mergeStringArrays(
    config.prompt_history,
    targetConfig?.prompt_history,
    projectConfig?.prompt_history,
  );
  config.api_profiles = mergeProfiles(config.api_profiles, targetConfig?.api_profiles);

  await writeJsonWithBackup(targetConfigPath, rewritePaths(config));
}

async function migrateWorkbenchRecords() {
  const oldRecordsPath = path.join(oldRuntimeRoot, "workbench_records.json");
  const targetRecordsPath = path.join(targetRoot, "workbench_records.json");
  const oldRecords = await readJsonIfExists(oldRecordsPath);
  const targetRecords = await readJsonIfExists(targetRecordsPath);
  const records = mergeWorkbenchRecords(
    normalizeRecordArray(oldRecords).map((record) => rewritePaths(record)),
    normalizeRecordArray(targetRecords).map((record) => rewritePaths(record)),
  );
  await writeJsonWithBackup(targetRecordsPath, records);
}

async function ensureStandardDirs() {
  for (const category of ASSET_CATEGORIES) {
    await ensureDir(path.join(targetAssetRoot, category));
  }
  for (const dirName of ["logs", ...TEMP_DIRS]) {
    await ensureDir(path.join(targetRoot, dirName));
  }
}

async function copyTree(sourceRoot, destRoot, options = {}) {
  if (!(await pathExists(sourceRoot))) {
    stats.missingSources += 1;
    console.log(`[migrate] source missing: ${sourceRoot}`);
    return;
  }

  const sourceStats = await fs.stat(sourceRoot);
  if (sourceStats.isFile()) {
    await copyFileUnique(sourceRoot, destRoot, options);
    return;
  }
  if (!sourceStats.isDirectory()) {
    return;
  }

  const entries = await fs.readdir(sourceRoot, { withFileTypes: true });
  for (const entry of entries) {
    const source = path.join(sourceRoot, entry.name);
    const destName = options.fileNamePrefix && entry.isFile()
      ? `${options.fileNamePrefix}${entry.name}`
      : entry.name;
    const dest = path.join(destRoot, destName);
    if (entry.isDirectory()) {
      await copyTree(source, dest, options);
    } else if (entry.isFile()) {
      await copyFileUnique(source, dest, options);
    }
  }
}

async function copyFileUnique(source, preferredDest, options = {}) {
  const sourceAbs = path.resolve(source);
  let dest = path.resolve(preferredDest);
  if (await pathExists(dest)) {
    if (await filesAreIdentical(sourceAbs, dest)) {
      stats.skippedIdentical += 1;
      if (options.mapPaths) {
        pathMap.set(sourceAbs, dest);
      }
      return dest;
    }
    dest = await nextAvailablePath(dest);
    stats.conflictsRenamed += 1;
  }

  if (apply) {
    await fs.mkdir(path.dirname(dest), { recursive: true });
    await fs.copyFile(sourceAbs, dest);
  }
  stats.copied += 1;
  if (options.mapPaths) {
    pathMap.set(sourceAbs, dest);
  }
  console.log(`${apply ? "[copy]" : "[would copy]"} ${sourceAbs} -> ${dest}`);
  return dest;
}

async function nextAvailablePath(filePath) {
  const parsed = path.parse(filePath);
  for (let index = 1; index < 10_000; index += 1) {
    const candidate = path.join(parsed.dir, `${parsed.name}.migrated_${index}${parsed.ext}`);
    if (!(await pathExists(candidate))) {
      return candidate;
    }
  }
  throw new Error(`无法生成不冲突的文件名: ${filePath}`);
}

async function filesAreIdentical(left, right) {
  const [leftStat, rightStat] = await Promise.all([fs.stat(left), fs.stat(right)]);
  if (leftStat.size !== rightStat.size) {
    return false;
  }
  const [leftHash, rightHash] = await Promise.all([hashFile(left), hashFile(right)]);
  return leftHash === rightHash;
}

async function hashFile(filePath) {
  const hash = createHash("sha256");
  const data = await fs.readFile(filePath);
  hash.update(data);
  return hash.digest("hex");
}

async function writeJsonWithBackup(filePath, value) {
  const dest = path.resolve(filePath);
  if (await pathExists(dest)) {
    const backup = backupPath(dest);
    if (apply) {
      await fs.mkdir(path.dirname(backup), { recursive: true });
      await fs.copyFile(dest, backup);
    }
    stats.backups += 1;
    console.log(`${apply ? "[backup]" : "[would backup]"} ${dest} -> ${backup}`);
  }

  if (apply) {
    await fs.mkdir(path.dirname(dest), { recursive: true });
    await fs.writeFile(dest, `${JSON.stringify(value, null, 2)}\n`);
  }
  stats.jsonWritten += 1;
  console.log(`${apply ? "[write]" : "[would write]"} ${dest}`);
}

function backupPath(filePath) {
  const parsed = path.parse(filePath);
  return path.join(
    parsed.dir,
    `${parsed.name}.before_portable_migration_${timestamp}${parsed.ext}`,
  );
}

function rewritePaths(value) {
  if (typeof value === "string") {
    return rewritePathString(value);
  }
  if (Array.isArray(value)) {
    return value.map((item) => rewritePaths(item));
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [key, rewritePaths(item)]),
    );
  }
  return value;
}

function rewritePathString(value) {
  const exact = pathMap.get(path.resolve(value));
  if (exact) {
    return exact;
  }

  const replacements = [
    [oldAssetRoot, targetAssetRoot],
    [oldRuntimeRoot, targetRoot],
  ];
  for (const [from, to] of replacements) {
    if (value === from) {
      return to;
    }
    if (value.startsWith(`${from}${path.sep}`)) {
      return path.join(to, path.relative(from, value));
    }
  }
  return value;
}

function mergeStringArrays(...arrays) {
  const seen = new Set();
  const merged = [];
  for (const array of arrays) {
    if (!Array.isArray(array)) {
      continue;
    }
    for (const value of array) {
      if (typeof value !== "string") {
        continue;
      }
      const trimmed = value.trim();
      if (!trimmed || seen.has(trimmed)) {
        continue;
      }
      seen.add(trimmed);
      merged.push(trimmed);
    }
  }
  return merged;
}

function mergeProfiles(primary, secondary) {
  const merged = [];
  const seen = new Set();
  for (const profile of [...asArray(primary), ...asArray(secondary)]) {
    if (!profile || typeof profile !== "object") {
      continue;
    }
    const id = typeof profile.id === "string" && profile.id.trim()
      ? profile.id.trim()
      : `api-profile-${merged.length + 1}`;
    if (seen.has(id)) {
      continue;
    }
    seen.add(id);
    merged.push({ ...profile, id });
  }
  return merged;
}

function mergeWorkbenchRecords(...recordGroups) {
  const merged = [];
  const indexes = new Map();
  for (const records of recordGroups) {
    for (const record of records) {
      const key = record.id || record.path;
      if (!key) {
        continue;
      }
      if (indexes.has(key)) {
        merged[indexes.get(key)] = { ...merged[indexes.get(key)], ...record };
      } else {
        indexes.set(key, merged.length);
        merged.push(record);
      }
    }
  }
  return merged;
}

function normalizeRecordArray(value) {
  return asArray(value)
    .filter((record) => record && typeof record === "object")
    .filter((record) => typeof record.path === "string" && record.path.trim())
    .map((record) => ({ ...record, path: record.path.trim() }));
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

async function readJsonIfExists(filePath) {
  if (!(await pathExists(filePath))) {
    return null;
  }
  const content = await fs.readFile(filePath, "utf8");
  return JSON.parse(content);
}

async function ensureDir(dir) {
  if (apply) {
    await fs.mkdir(dir, { recursive: true });
  }
}

async function pathExists(filePath) {
  return existsSync(filePath);
}

function timestampForFile() {
  return new Date()
    .toISOString()
    .replaceAll("-", "")
    .replaceAll(":", "")
    .replace(/\.\d+Z$/, "");
}

async function printVerification() {
  console.log("[migrate] summary", JSON.stringify(stats));
  if (!apply) {
    console.log("[migrate] dry run complete; rerun with --apply to migrate data.");
    return;
  }

  const remainingOldPaths = [];
  for (const fileName of ["config.json", "workbench_records.json"]) {
    const filePath = path.join(targetRoot, fileName);
    if (!(await pathExists(filePath))) {
      continue;
    }
    const content = await fs.readFile(filePath, "utf8");
    for (const oldRoot of [oldAssetRoot, oldRuntimeRoot]) {
      if (content.includes(oldRoot)) {
        remainingOldPaths.push(`${fileName}: ${oldRoot}`);
      }
    }
  }

  if (remainingOldPaths.length > 0) {
    console.warn("[migrate] old paths still found:");
    for (const item of remainingOldPaths) {
      console.warn(`  ${item}`);
    }
  } else {
    console.log("[migrate] active JSON files no longer reference old data roots.");
  }

  await printCounts();
}

async function printCounts() {
  console.log("[migrate] target file counts:");
  for (const category of ASSET_CATEGORIES) {
    const count = await countFiles(path.join(targetAssetRoot, category));
    console.log(`  assets/${category}: ${count}`);
  }
  for (const dirName of TEMP_DIRS) {
    const count = await countFiles(path.join(targetRoot, dirName));
    console.log(`  ${dirName}: ${count}`);
  }
  console.log(`  logs: ${await countFiles(path.join(targetRoot, "logs"))}`);
}

async function countFiles(root) {
  if (!(await pathExists(root))) {
    return 0;
  }
  const stat = await fs.stat(root);
  if (stat.isFile()) {
    return 1;
  }
  if (!stat.isDirectory()) {
    return 0;
  }
  let count = 0;
  const entries = await fs.readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    count += await countFiles(path.join(root, entry.name));
  }
  return count;
}
