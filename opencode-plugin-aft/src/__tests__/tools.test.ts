import { describe, test, expect, afterEach } from "bun:test";
import { BinaryBridge } from "../bridge.js";
import { readingTools } from "../tools/reading.js";
import { editingTools } from "../tools/editing.js";
import { safetyTools } from "../tools/safety.js";
import { transactionTools } from "../tools/transaction.js";
import { navigationTools } from "../tools/navigation.js";
import { refactoringTools } from "../tools/refactoring.js";
import { resolve } from "node:path";
import { mkdtemp, rm, readFile, writeFile, mkdir, cp } from "node:fs/promises";
import { tmpdir } from "node:os";

const BINARY_PATH = resolve(import.meta.dir, "../../../target/debug/aft");
const PROJECT_CWD = resolve(import.meta.dir, "../../..");
const FIXTURE_FILE = resolve(PROJECT_CWD, "tests/fixtures/sample.ts");

const TEST_TIMEOUT_MS = 10_000;

describe("Tool round-trips", () => {
  let bridge: BinaryBridge;
  let tmpDir: string | null = null;

  // Fresh bridge per test — each test is independent
  const createBridge = () => {
    bridge = new BinaryBridge(BINARY_PATH, PROJECT_CWD, {
      timeoutMs: TEST_TIMEOUT_MS,
    });
    return bridge;
  };

  afterEach(async () => {
    if (bridge) {
      await bridge.shutdown();
    }
    if (tmpDir) {
      await rm(tmpDir, { recursive: true, force: true });
      tmpDir = null;
    }
  });

  test("outline tool returns entries for fixture file with known symbols", async () => {
    createBridge();
    const tools = readingTools(bridge);

    const resultStr = await tools.outline.execute({ file: FIXTURE_FILE });
    const result = JSON.parse(resultStr);

    expect(result.ok).toBe(true);
    expect(Array.isArray(result.entries)).toBe(true);
    expect(result.entries.length).toBeGreaterThan(0);

    // Verify known symbols from the fixture
    const names = result.entries.map((e: any) => e.name);
    expect(names).toContain("greet");
    expect(names).toContain("add");
    expect(names).toContain("UserService");
    expect(names).toContain("Config");
    expect(names).toContain("Status");
    expect(names).toContain("UserId");
    expect(names).toContain("internalHelper");

    // Verify structure of an entry
    const greetEntry = result.entries.find((e: any) => e.name === "greet");
    expect(greetEntry.kind).toBe("function");
    expect(greetEntry.exported).toBe(true);
    expect(greetEntry.range).toBeDefined();
    expect(greetEntry.range.start_line).toBeDefined();
    expect(greetEntry.range.end_line).toBeDefined();
  });

  test("write tool creates a temp file and returns syntax_valid", async () => {
    createBridge();
    const tools = editingTools(bridge);
    tmpDir = await mkdtemp(resolve(tmpdir(), "aft-test-"));

    const filePath = resolve(tmpDir, "written.ts");
    const content = 'export function greetWorld(): string {\n  return "hello world";\n}\n';

    const resultStr = await tools.write.execute({
      file: filePath,
      content,
      create_dirs: false,
    });
    const result = JSON.parse(resultStr);

    expect(result.ok).toBe(true);
    expect(result.syntax_valid).toBe(true);
    expect(result.file).toBe(filePath);

    // Verify the file was actually written
    const fileContent = await readFile(filePath, "utf-8");
    expect(fileContent).toBe(content);
  });

  test("edit_symbol replaces a function and returns backup_id and syntax_valid", async () => {
    createBridge();
    const tools = editingTools(bridge);
    tmpDir = await mkdtemp(resolve(tmpdir(), "aft-test-"));

    const filePath = resolve(tmpDir, "editable.ts");
    const original = 'export function hello(): string {\n  return "hi";\n}\n';

    // First write the file
    await tools.write.execute({ file: filePath, content: original });

    // Now replace the symbol
    const newContent = 'export function hello(): string {\n  return "world";\n}\n';
    const resultStr = await tools.edit_symbol.execute({
      file: filePath,
      symbol: "hello",
      operation: "replace",
      content: newContent,
    });
    const result = JSON.parse(resultStr);

    expect(result.ok).toBe(true);
    expect(result.backup_id).toBeDefined();
    expect(typeof result.backup_id).toBe("string");
    expect(result.symbol).toBe("hello");
    expect(result.operation).toBe("replace");

    // Verify the file was actually changed
    const fileContent = await readFile(filePath, "utf-8");
    expect(fileContent).toContain("world");
    expect(fileContent).not.toContain('"hi"');
  });

  test("undo restores the file after edit_symbol", async () => {
    createBridge();
    const editTools = editingTools(bridge);
    const undoTools = safetyTools(bridge);
    tmpDir = await mkdtemp(resolve(tmpdir(), "aft-test-"));

    const filePath = resolve(tmpDir, "undoable.ts");
    const original = 'export function greet(name: string): string {\n  return `Hello, ${name}!`;\n}\n';

    // Write original file
    await editTools.write.execute({ file: filePath, content: original });

    // Edit the symbol
    const replacement = 'export function greet(name: string): string {\n  return `Goodbye, ${name}!`;\n}\n';
    const editResult = JSON.parse(
      await editTools.edit_symbol.execute({
        file: filePath,
        symbol: "greet",
        operation: "replace",
        content: replacement,
      }),
    );
    expect(editResult.ok).toBe(true);

    // Verify file was changed
    let content = await readFile(filePath, "utf-8");
    expect(content).toContain("Goodbye");

    // Undo the edit
    const undoResult = JSON.parse(
      await undoTools.undo.execute({ file: filePath }),
    );
    expect(undoResult.ok).toBe(true);
    expect(undoResult.backup_id).toBeDefined();

    // Verify file was restored
    content = await readFile(filePath, "utf-8");
    expect(content).toContain("Hello");
    expect(content).not.toContain("Goodbye");
  });

  test("write dry_run returns diff without modifying file", async () => {
    createBridge();
    const tools = editingTools(bridge);
    tmpDir = await mkdtemp(resolve(tmpdir(), "aft-test-"));

    const filePath = resolve(tmpDir, "dryrun.ts");
    const original = 'export function hello(): string {\n  return "hi";\n}\n';

    // Write the original file first
    await tools.write.execute({ file: filePath, content: original });

    // Now dry-run a write with different content
    const newContent = 'export function hello(): string {\n  return "world";\n}\n';
    const resultStr = await tools.write.execute({
      file: filePath,
      content: newContent,
      dry_run: true,
    });
    const result = JSON.parse(resultStr);

    expect(result.ok).toBe(true);
    expect(result.dry_run).toBe(true);
    expect(typeof result.diff).toBe("string");
    expect(result.diff).toContain("-");
    expect(result.diff).toContain("+");
    expect(result.syntax_valid).toBe(true);

    // Verify file was NOT modified
    const fileContent = await readFile(filePath, "utf-8");
    expect(fileContent).toBe(original);
  });

  test("transaction success applies multiple file writes", async () => {
    createBridge();
    const tools = transactionTools(bridge);
    tmpDir = await mkdtemp(resolve(tmpdir(), "aft-test-"));

    const file1 = resolve(tmpDir, "a.ts");
    const file2 = resolve(tmpDir, "b.ts");

    const resultStr = await tools.transaction.execute({
      operations: [
        { file: file1, command: "write", content: 'export const a = 1;\n' },
        { file: file2, command: "write", content: 'export const b = 2;\n' },
      ],
    });
    const result = JSON.parse(resultStr);

    expect(result.ok).toBe(true);
    expect(result.files_modified).toBe(2);
    expect(Array.isArray(result.results)).toBe(true);
    expect(result.results.length).toBe(2);

    // Verify both files were created
    const content1 = await readFile(file1, "utf-8");
    const content2 = await readFile(file2, "utf-8");
    expect(content1).toContain("a = 1");
    expect(content2).toContain("b = 2");
  });

  test("transaction rollback on syntax error", async () => {
    createBridge();
    const editTools = editingTools(bridge);
    const tools = transactionTools(bridge);
    tmpDir = await mkdtemp(resolve(tmpdir(), "aft-test-"));

    // Create a valid file that should be restored on rollback
    const existingFile = resolve(tmpDir, "existing.ts");
    const originalContent = 'export const x = 1;\n';
    await editTools.write.execute({ file: existingFile, content: originalContent });

    // Transaction: write valid content to existing file, then write broken syntax to new file
    const brokenFile = resolve(tmpDir, "broken.ts");
    const resultStr = await tools.transaction.execute({
      operations: [
        { file: existingFile, command: "write", content: 'export const x = 999;\n' },
        { file: brokenFile, command: "write", content: 'export const {{{broken = ;\n' },
      ],
    });
    const result = JSON.parse(resultStr);

    // Transaction should fail due to syntax error
    expect(result.ok).toBe(false);
    expect(result.code).toBe("transaction_failed");
    expect(Array.isArray(result.rolled_back)).toBe(true);

    // Existing file should be restored to original content
    const restoredContent = await readFile(existingFile, "utf-8");
    expect(restoredContent).toBe(originalContent);
  });
});

describe("move_symbol round-trip", () => {
  let bridge: BinaryBridge;
  let tmpDir: string;

  const TEST_TIMEOUT_MS = 15_000;

  afterEach(async () => {
    if (bridge) await bridge.shutdown();
    if (tmpDir) await rm(tmpDir, { recursive: true, force: true });
  });

  test("aft_move_symbol moves a function and rewires consumer import", async () => {
    bridge = new BinaryBridge(BINARY_PATH, PROJECT_CWD, {
      timeoutMs: TEST_TIMEOUT_MS,
    });

    // Create temp project with source, consumer, and destination
    tmpDir = await mkdtemp(resolve(tmpdir(), "aft-move-"));

    const sourceFile = resolve(tmpDir, "service.ts");
    const consumerFile = resolve(tmpDir, "consumer.ts");
    const destFile = resolve(tmpDir, "utils.ts");

    await writeFile(
      sourceFile,
      [
        'export function formatDate(date: Date): string {',
        '  return date.toISOString();',
        '}',
        '',
        'export function otherFn(): void {}',
        '',
      ].join('\n'),
    );

    await writeFile(
      consumerFile,
      [
        "import { formatDate } from './service';",
        '',
        'export function render(d: Date): string {',
        '  return formatDate(d);',
        '}',
        '',
      ].join('\n'),
    );

    // Configure the bridge with the temp dir as project root
    const navTools = navigationTools(bridge);
    const configResult = JSON.parse(
      await navTools.aft_configure.execute({ project_root: tmpDir }),
    );
    expect(configResult.ok).toBe(true);

    // Move formatDate from service.ts to utils.ts
    const refTools = refactoringTools(bridge);
    const moveResult = JSON.parse(
      await refTools.aft_move_symbol.execute({
        file: sourceFile,
        symbol: "formatDate",
        destination: destFile,
      }),
    );

    expect(moveResult.ok).toBe(true);
    expect(moveResult.files_modified).toBeGreaterThanOrEqual(2);

    // Verify response structure includes expected diagnostic fields
    expect(moveResult.consumers_updated).toBeDefined();
    expect(moveResult.checkpoint_name).toBeDefined();

    // Verify symbol was actually moved on disk
    const sourceContent = await readFile(sourceFile, "utf-8");
    expect(sourceContent).not.toContain("formatDate");
    expect(sourceContent).toContain("otherFn");

    const destContent = await readFile(destFile, "utf-8");
    expect(destContent).toContain("formatDate");

    // Verify consumer import was rewired
    const consumerContent = await readFile(consumerFile, "utf-8");
    expect(consumerContent).toContain("./utils");
    expect(consumerContent).not.toContain("./service");
  }, TEST_TIMEOUT_MS);
});

describe("extract_function round-trip", () => {
  let bridge: BinaryBridge;
  let tmpDir: string;

  const TEST_TIMEOUT_MS = 15_000;

  afterEach(async () => {
    if (bridge) await bridge.shutdown();
    if (tmpDir) await rm(tmpDir, { recursive: true, force: true });
  });

  test("aft_extract_function extracts code range into a new function with parameters", async () => {
    bridge = new BinaryBridge(BINARY_PATH, PROJECT_CWD, {
      timeoutMs: TEST_TIMEOUT_MS,
    });

    tmpDir = await mkdtemp(resolve(tmpdir(), "aft-extract-"));

    const filePath = resolve(tmpDir, "source.ts");
    await writeFile(
      filePath,
      [
        'function processData(items: string[], prefix: string): string {',
        '  const filtered = items.filter(item => item.length > 0);',
        '  const mapped = filtered.map(item => prefix + item);',
        '  const result = mapped.join(", ");',
        '  return result;',
        '}',
        '',
      ].join('\n'),
    );

    // Configure
    const navTools = navigationTools(bridge);
    const configResult = JSON.parse(
      await navTools.aft_configure.execute({ project_root: tmpDir }),
    );
    expect(configResult.ok).toBe(true);

    // Extract lines 1-3 (the filtering and mapping logic)
    const refTools = refactoringTools(bridge);
    const result = JSON.parse(
      await refTools.aft_extract_function.execute({
        file: filePath,
        name: "filterAndMap",
        start_line: 1,
        end_line: 4,
        dry_run: true,
      }),
    );

    expect(result.ok).toBe(true);
    expect(result.dry_run).toBe(true);
    expect(Array.isArray(result.parameters)).toBe(true);
    expect(result.parameters.length).toBeGreaterThan(0);
    expect(result.return_type).toBeDefined();
    expect(typeof result.diff).toBe("string");
  }, TEST_TIMEOUT_MS);
});

describe("inline_symbol round-trip", () => {
  let bridge: BinaryBridge;
  let tmpDir: string;

  const TEST_TIMEOUT_MS = 15_000;

  afterEach(async () => {
    if (bridge) await bridge.shutdown();
    if (tmpDir) await rm(tmpDir, { recursive: true, force: true });
  });

  test("aft_inline_symbol inlines a function call and returns substitution info", async () => {
    bridge = new BinaryBridge(BINARY_PATH, PROJECT_CWD, {
      timeoutMs: TEST_TIMEOUT_MS,
    });

    tmpDir = await mkdtemp(resolve(tmpdir(), "aft-inline-"));

    const filePath = resolve(tmpDir, "source.ts");
    await writeFile(
      filePath,
      [
        'function helper(a: number, b: number): number {',
        '  return a + b;',
        '}',
        '',
        'function main() {',
        '  const result = helper(10, 20);',
        '  console.log(result);',
        '}',
        '',
      ].join('\n'),
    );

    // Configure
    const navTools = navigationTools(bridge);
    const configResult = JSON.parse(
      await navTools.aft_configure.execute({ project_root: tmpDir }),
    );
    expect(configResult.ok).toBe(true);

    // Inline helper at line 5 (const result = helper(10, 20))
    const refTools = refactoringTools(bridge);
    const result = JSON.parse(
      await refTools.aft_inline_symbol.execute({
        file: filePath,
        symbol: "helper",
        call_site_line: 5,
      }),
    );

    expect(result.ok).toBe(true);
    expect(result.symbol).toBe("helper");
    expect(result.call_context).toBe("assignment");
    expect(result.substitutions).toBeGreaterThan(0);

    // Verify file was modified
    const content = await readFile(filePath, "utf-8");
    expect(content).not.toContain("helper(10, 20)");
  }, TEST_TIMEOUT_MS);
});
