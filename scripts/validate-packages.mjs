#!/usr/bin/env node

/**
 * validate-packages.mjs
 *
 * Validates all 6 AFT npm package.json files:
 * - 5 platform packages under npm/{platform}/
 * - 1 root @cortexkit/aft-opencode package at opencode-plugin-aft/
 *
 * Checks: os/cpu fields match directory, preferUnplugged, bin field,
 * optionalDependencies in core, version alignment, required fields.
 *
 * Exit 0 = all pass. Exit 1 = failures printed to stderr.
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");

const PLATFORMS = [
  { dir: "darwin-arm64", os: "darwin", cpu: "arm64" },
  { dir: "darwin-x64", os: "darwin", cpu: "x64" },
  { dir: "linux-arm64", os: "linux", cpu: "arm64" },
  { dir: "linux-x64", os: "linux", cpu: "x64" },
  { dir: "win32-x64", os: "win32", cpu: "x64" },
];

const errors = [];

function fail(pkg, msg) {
  errors.push(`[${pkg}] ${msg}`);
}

function readPkg(path) {
  try {
    return JSON.parse(readFileSync(path, "utf-8"));
  } catch (e) {
    errors.push(`Cannot read ${path}: ${e.message}`);
    return null;
  }
}

// --- Validate platform packages ---

const platformVersions = [];

for (const { dir, os, cpu } of PLATFORMS) {
  const pkgPath = join(root, "packages", "npm", dir, "package.json");
  const pkg = readPkg(pkgPath);
  if (!pkg) continue;

  const label = `@cortexkit/aft-${dir}`;

  // Required fields
  if (!pkg.name) fail(label, "missing 'name'");
  if (!pkg.version) fail(label, "missing 'version'");
  if (pkg.name && pkg.name !== `@cortexkit/aft-${dir}`) {
    fail(label, `name should be '@cortexkit/aft-${dir}', got '${pkg.name}'`);
  }

  // os/cpu arrays
  if (!Array.isArray(pkg.os) || pkg.os.length !== 1 || pkg.os[0] !== os) {
    fail(label, `os should be ["${os}"], got ${JSON.stringify(pkg.os)}`);
  }
  if (!Array.isArray(pkg.cpu) || pkg.cpu.length !== 1 || pkg.cpu[0] !== cpu) {
    fail(label, `cpu should be ["${cpu}"], got ${JSON.stringify(pkg.cpu)}`);
  }

  // preferUnplugged
  if (pkg.preferUnplugged !== true) {
    fail(label, "missing or false 'preferUnplugged: true'");
  }

  // bin field
  if (!pkg.bin || typeof pkg.bin !== "object") {
    fail(label, "missing 'bin' field");
  } else if (!pkg.bin.aft) {
    fail(label, "bin field missing 'aft' entry");
  }

  if (pkg.version) platformVersions.push({ name: label, version: pkg.version });
}

// --- Validate @cortexkit/aft-opencode ---

const corePath = join(root, "packages", "opencode-plugin", "package.json");
const core = readPkg(corePath);

if (core) {
  const label = "@cortexkit/aft-opencode";

  // Required fields
  if (!core.name) fail(label, "missing 'name'");
  if (!core.version) fail(label, "missing 'version'");
  if (core.name && core.name !== "@cortexkit/aft-opencode") {
    fail(label, `name should be '@cortexkit/aft-opencode', got '${core.name}'`);
  }

  // optionalDependencies must list all 5 platform packages
  const optDeps = core.optionalDependencies || {};
  for (const { dir } of PLATFORMS) {
    const depName = `@cortexkit/aft-${dir}`;
    if (!(depName in optDeps)) {
      fail(label, `optionalDependencies missing '${depName}'`);
    }
  }

  if (core.version) platformVersions.push({ name: label, version: core.version });
}

// --- Version alignment ---

if (platformVersions.length > 1) {
  const first = platformVersions[0];
  for (let i = 1; i < platformVersions.length; i++) {
    const other = platformVersions[i];
    if (other.version !== first.version) {
      fail("version-alignment", `${first.name}@${first.version} != ${other.name}@${other.version}`);
    }
  }
}

// Also check that optionalDependencies versions match the core version
if (core?.version && core.optionalDependencies) {
  for (const [depName, depVersion] of Object.entries(core.optionalDependencies)) {
    if (depVersion !== core.version) {
      fail(
        "version-alignment",
        `@cortexkit/aft-opencode optionalDependencies['${depName}'] is '${depVersion}' but core version is '${core.version}'`,
      );
    }
  }
}

// --- Report ---

if (errors.length > 0) {
  console.error("Package validation FAILED:\n");
  for (const err of errors) {
    console.error(`  ✗ ${err}`);
  }
  console.error(`\n${errors.length} error(s) found.`);
  process.exit(1);
} else {
  const count = PLATFORMS.length + 1;
  console.log(`✓ All ${count} packages validated successfully.`);
  console.log(`  Versions aligned at ${platformVersions[0]?.version || "unknown"}`);
  console.log("  Platform os/cpu fields correct");
  console.log("  preferUnplugged set on all platform packages");
  console.log("  optionalDependencies complete in @cortexkit/aft-opencode");
  process.exit(0);
}
