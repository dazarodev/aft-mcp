#!/usr/bin/env node
"use strict";

/**
 * postinstall: ensure the aft-mcp binary is available.
 * Checks npm platform package first, then downloads from GitHub Releases.
 */

const {
  chmodSync,
  existsSync,
  mkdirSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} = require("fs");
const { createHash } = require("crypto");
const { get } = require("https");
const { homedir } = require("os");
const path = require("path");
const { version } = require("./package.json");

const REPO = "dazarodev/aft-mcp";
const isWindows = process.platform === "win32";
const binName = isWindows ? "aft-mcp.exe" : "aft-mcp";
const key = `${process.platform}-${process.arch}`;

const PLATFORMS = {
  "darwin-arm64": "@dazarodev/aft-mcp-darwin-arm64",
  "darwin-x64": "@dazarodev/aft-mcp-darwin-x64",
  "linux-arm64": "@dazarodev/aft-mcp-linux-arm64",
  "linux-x64": "@dazarodev/aft-mcp-linux-x64",
  "win32-x64": "@dazarodev/aft-mcp-win32-x64",
};

const ASSET_MAP = {
  "darwin-arm64": "aft-mcp-darwin-arm64",
  "darwin-x64": "aft-mcp-darwin-x64",
  "linux-arm64": "aft-mcp-linux-arm64",
  "linux-x64": "aft-mcp-linux-x64",
  "win32-x64": "aft-mcp-win32-x64.exe",
};

function getCacheDir() {
  if (isWindows) {
    const base =
      process.env.LOCALAPPDATA ||
      process.env.APPDATA ||
      path.join(homedir(), "AppData", "Local");
    return path.join(base, "aft-mcp", "bin");
  }
  const base = process.env.XDG_CACHE_HOME || path.join(homedir(), ".cache");
  return path.join(base, "aft-mcp", "bin");
}

function findPlatformPackage() {
  try {
    const pkg = PLATFORMS[key];
    if (!pkg) return null;
    const pkgDir = path.dirname(require.resolve(`${pkg}/package.json`));
    const p = path.join(pkgDir, "bin", binName);
    return existsSync(p) ? p : null;
  } catch {}
  return null;
}

function download(url) {
  return new Promise((resolve, reject) => {
    get(url, (res) => {
      if (
        res.statusCode >= 300 &&
        res.statusCode < 400 &&
        res.headers.location
      ) {
        res.resume();
        download(res.headers.location).then(resolve, reject);
        return;
      }
      if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode}`));
        return;
      }
      const chunks = [];
      res.on("data", (c) => chunks.push(c));
      res.on("end", () => resolve(Buffer.concat(chunks)));
      res.on("error", reject);
    }).on("error", reject);
  });
}

async function main() {
  if (!PLATFORMS[key]) {
    console.error(`aft-mcp: unsupported platform ${key}`);
    process.exit(0); // don't fail install on unsupported platforms
  }

  // Already have it from platform package?
  if (findPlatformPackage()) return;

  // Already cached?
  const tag = `v${version}`;
  const cacheDir = path.join(getCacheDir(), tag);
  const binaryPath = path.join(cacheDir, binName);
  if (existsSync(binaryPath)) return;

  // Download
  const asset = ASSET_MAP[key];
  const url = `https://github.com/${REPO}/releases/download/${tag}/${asset}`;
  const checksumUrl = `https://github.com/${REPO}/releases/download/${tag}/checksums.sha256`;

  console.log(`Downloading aft-mcp ${tag} for ${key}...`);
  mkdirSync(cacheDir, { recursive: true });
  const tmpPath = `${binaryPath}.tmp`;

  try {
    const [binaryBuf, checksumBuf] = await Promise.all([
      download(url),
      download(checksumUrl).catch(() => null),
    ]);

    if (checksumBuf) {
      const lines = checksumBuf.toString().split("\n");
      const expected = lines
        .map((l) => l.trim().match(/^([0-9a-f]{64})\s+(.+)$/))
        .find((m) => m && m[2] === asset);
      if (expected) {
        const actual = createHash("sha256").update(binaryBuf).digest("hex");
        if (actual !== expected[1]) {
          throw new Error(
            `Checksum mismatch: expected ${expected[1]}, got ${actual}`,
          );
        }
      }
    }

    writeFileSync(tmpPath, binaryBuf);
    if (!isWindows) chmodSync(tmpPath, 0o755);
    renameSync(tmpPath, binaryPath);
    console.log("aft-mcp binary ready.");
  } catch (err) {
    if (existsSync(tmpPath)) {
      try {
        unlinkSync(tmpPath);
      } catch {}
    }
    console.error(
      `aft-mcp: download failed (${err.message}). Install manually.`,
    );
    // Don't fail the install
    process.exit(0);
  }
}

main().catch(() => process.exit(0));
