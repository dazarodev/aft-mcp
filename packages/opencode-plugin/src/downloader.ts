/**
 * Auto-download the AFT binary from GitHub releases.
 *
 * Resolution order (in resolver.ts):
 *   1. Cached binary in ~/.cache/aft/bin/
 *   2. npm platform package (@cortexkit/aft-darwin-arm64, etc.)
 *   3. PATH lookup (which aft)
 *   4. ~/.cargo/bin/aft
 *   5. Auto-download from GitHub releases (this module)
 *
 * Cache dir respects XDG_CACHE_HOME on Linux/macOS and LOCALAPPDATA on Windows.
 */

import { chmodSync, existsSync, mkdirSync, unlinkSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const REPO = "nichochar/opencode-sfm";
const TAG = "[aft-downloader]";

/** Platform → GitHub release asset suffix */
const PLATFORM_MAP: Record<string, string> = {
  "darwin-arm64": "aft-darwin-arm64",
  "darwin-x64": "aft-darwin-x64",
  "linux-arm64": "aft-linux-arm64",
  "linux-x64": "aft-linux-x64",
  "win32-x64": "aft-win32-x64.exe",
};

/** Get the cache directory, respecting XDG_CACHE_HOME / LOCALAPPDATA. */
export function getCacheDir(): string {
  if (process.platform === "win32") {
    const localAppData = process.env.LOCALAPPDATA || process.env.APPDATA;
    const base = localAppData || join(homedir(), "AppData", "Local");
    return join(base, "aft", "bin");
  }

  const base = process.env.XDG_CACHE_HOME || join(homedir(), ".cache");
  return join(base, "aft", "bin");
}

/** Binary name for the current platform. */
export function getBinaryName(): string {
  return process.platform === "win32" ? "aft.exe" : "aft";
}

/** Return the cached binary path if it exists, otherwise null. */
export function getCachedBinaryPath(): string | null {
  const binaryPath = join(getCacheDir(), getBinaryName());
  return existsSync(binaryPath) ? binaryPath : null;
}

/**
 * Download the AFT binary for the current platform from GitHub releases.
 *
 * @param version - Git tag to download from (e.g. "v0.1.0"). If omitted,
 *   fetches the latest release tag via the GitHub API.
 * @returns Absolute path to the downloaded binary, or null on failure.
 */
export async function downloadBinary(version?: string): Promise<string | null> {
  const platformKey = `${process.platform}-${process.arch}`;
  const assetName = PLATFORM_MAP[platformKey];

  if (!assetName) {
    console.error(`${TAG} Unsupported platform: ${platformKey}`);
    return null;
  }

  const cacheDir = getCacheDir();
  const binaryName = getBinaryName();
  const binaryPath = join(cacheDir, binaryName);

  // Already cached
  if (existsSync(binaryPath)) {
    return binaryPath;
  }

  // Resolve version if not provided
  const tag = version ?? (await fetchLatestTag());
  if (!tag) {
    console.error(`${TAG} Could not determine latest release version.`);
    return null;
  }

  const downloadUrl = `https://github.com/${REPO}/releases/download/${tag}/${assetName}`;

  console.error(`${TAG} Downloading AFT binary (${tag}) for ${platformKey}...`);

  try {
    // Ensure cache directory exists
    if (!existsSync(cacheDir)) {
      mkdirSync(cacheDir, { recursive: true });
    }

    // Download
    const response = await fetch(downloadUrl, { redirect: "follow" });
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText} (${downloadUrl})`);
    }

    const arrayBuffer = await response.arrayBuffer();

    // Write to a temp file first, then rename (atomic-ish)
    const tmpPath = `${binaryPath}.tmp`;
    const { writeFileSync } = await import("node:fs");
    writeFileSync(tmpPath, Buffer.from(arrayBuffer));

    // Make executable
    if (process.platform !== "win32") {
      chmodSync(tmpPath, 0o755);
    }

    // Atomic rename
    const { renameSync } = await import("node:fs");
    renameSync(tmpPath, binaryPath);

    console.error(`${TAG} AFT binary ready at ${binaryPath}`);
    return binaryPath;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`${TAG} Failed to download AFT binary: ${msg}`);

    // Clean up partial download
    const tmpPath = `${binaryPath}.tmp`;
    if (existsSync(tmpPath)) {
      try {
        unlinkSync(tmpPath);
      } catch {
        // ignore cleanup failure
      }
    }

    return null;
  }
}

/**
 * Ensure the AFT binary is available: check cache, then download if needed.
 * This is the main entry point called by the resolver.
 */
export async function ensureBinary(version?: string): Promise<string | null> {
  const cached = getCachedBinaryPath();
  if (cached) return cached;
  return downloadBinary(version);
}

/** Fetch the latest release tag from GitHub API. */
async function fetchLatestTag(): Promise<string | null> {
  try {
    const response = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { Accept: "application/vnd.github.v3+json" },
    });
    if (!response.ok) return null;
    const data = (await response.json()) as { tag_name?: string };
    return data.tag_name ?? null;
  } catch {
    return null;
  }
}
