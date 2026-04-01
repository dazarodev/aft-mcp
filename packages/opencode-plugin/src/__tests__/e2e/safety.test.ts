/// <reference path="../../bun-test.d.ts" />

import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { writeFile } from "node:fs/promises";
import {
  cleanupHarnesses,
  createHarness,
  type E2EHarness,
  type PreparedBinary,
  prepareBinary,
  readTextFile,
} from "./helpers.js";

const initialBinary = await prepareBinary();
const maybeDescribe = describe.skipIf(!initialBinary.binaryPath);

maybeDescribe("e2e safety commands", () => {
  let preparedBinary: PreparedBinary = initialBinary;
  const harnesses: E2EHarness[] = [];

  beforeAll(async () => {
    preparedBinary = await prepareBinary();
  });

  afterEach(async () => {
    await cleanupHarnesses(harnesses);
  });

  async function harness(): Promise<E2EHarness> {
    const created = await createHarness(preparedBinary);
    harnesses.push(created);
    return created;
  }

  test("creates and restores a checkpoint", async () => {
    const h = await harness();
    const filePath = h.path("sample.ts");
    const original = await readTextFile(filePath);

    const checkpoint = await h.bridge.send("checkpoint", { name: "safe-point", files: [filePath] });
    await h.bridge.send("write", { file: filePath, content: "export const changed = true;\n" });
    const restore = await h.bridge.send("restore_checkpoint", { name: "safe-point" });

    expect(checkpoint.success).toBe(true);
    expect(restore.success).toBe(true);
    expect(await readTextFile(filePath)).toBe(original);
  });

  test("undo reverts an edit", async () => {
    const h = await harness();
    const filePath = h.path("with-errors.ts");
    const original = await readTextFile(filePath);

    const edit = await h.bridge.send("edit_match", {
      file: filePath,
      match: "pending",
      replacement: "ready",
      occurrence: 0,
    });
    const undo = await h.bridge.send("undo", { file: filePath });

    expect(edit.success).toBe(true);
    expect(undo.success).toBe(true);
    expect(await readTextFile(filePath)).toBe(original);
  });

  test("history lists prior snapshots", async () => {
    const h = await harness();
    const filePath = h.path("history.txt");
    await writeFile(filePath, "v1\n");

    await h.bridge.send("write", { file: filePath, content: "v2\n" });
    await h.bridge.send("write", { file: filePath, content: "v3\n" });
    const history = await h.bridge.send("edit_history", { file: filePath });

    expect(history.success).toBe(true);
    expect((history.entries as Array<Record<string, unknown>>).length).toBeGreaterThanOrEqual(2);
  });

  test("multiple undos walk back the stack", async () => {
    const h = await harness();
    const filePath = h.path("undo-stack.txt");
    await writeFile(filePath, "v1\n");

    await h.bridge.send("write", { file: filePath, content: "v2\n" });
    await h.bridge.send("write", { file: filePath, content: "v3\n" });
    await h.bridge.send("undo", { file: filePath });
    expect(await readTextFile(filePath)).toBe("v2\n");

    await h.bridge.send("undo", { file: filePath });
    expect(await readTextFile(filePath)).toBe("v1\n");
  });

  test("list_checkpoints returns created checkpoints", async () => {
    const h = await harness();
    const filePath = h.path("sample.ts");

    await h.bridge.send("checkpoint", { name: "one", files: [filePath] });
    await h.bridge.send("checkpoint", { name: "two", files: [filePath] });
    const response = await h.bridge.send("list_checkpoints");

    expect(response.success).toBe(true);
    const checkpoints = response.checkpoints as Array<Record<string, unknown>>;
    expect(checkpoints.some((checkpoint) => checkpoint.name === "one")).toBe(true);
    expect(checkpoints.some((checkpoint) => checkpoint.name === "two")).toBe(true);
  });
});
