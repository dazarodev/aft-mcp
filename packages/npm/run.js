#!/usr/bin/env node
"use strict";

const { execFileSync } = require("child_process");
const { accessSync, constants } = require("fs");
const path = require("path");

const PLATFORMS = {
  "darwin-arm64": "@dazarodev/aft-mcp-darwin-arm64",
  "darwin-x64": "@dazarodev/aft-mcp-darwin-x64",
  "linux-arm64": "@dazarodev/aft-mcp-linux-arm64",
  "linux-x64": "@dazarodev/aft-mcp-linux-x64",
  "win32-x64": "@dazarodev/aft-mcp-win32-x64",
};

const key = `${process.platform}-${process.arch}`;
const pkg = PLATFORMS[key];

if (!pkg) {
  process.stderr.write(
    `Unsupported platform: ${key}\nSupported: ${Object.keys(PLATFORMS).join(", ")}\n`,
  );
  process.exit(1);
}

const isWindows = process.platform === "win32";
const binName = isWindows ? "aft-mcp.exe" : "aft-mcp";

let binPath;
try {
  const pkgDir = path.dirname(require.resolve(`${pkg}/package.json`));
  binPath = path.join(pkgDir, "bin", binName);
} catch {
  process.stderr.write(
    `Platform package ${pkg} not found.\nRun: npm install aft-mcp to reinstall.\n`,
  );
  process.exit(1);
}

try {
  accessSync(binPath, constants.X_OK);
} catch {
  process.stderr.write(
    `Binary not found or not executable: ${binPath}\nRun: npm install aft-mcp to reinstall.\n`,
  );
  process.exit(1);
}

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: "inherit" });
} catch (err) {
  if (err.signal) {
    process.stderr.write(`aft-mcp crashed with signal ${err.signal}\n`);
    process.exit(1);
  }
  if (err.status != null) {
    process.exit(err.status);
  }
  process.stderr.write(`Failed to run aft-mcp: ${err.message}\n`);
  process.exit(1);
}
