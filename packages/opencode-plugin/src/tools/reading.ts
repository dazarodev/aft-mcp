import type { ToolDefinition } from "@opencode-ai/plugin";
import { tool } from "@opencode-ai/plugin";
import { readdir } from "node:fs/promises";
import { join, resolve, extname } from "node:path";
import type { PluginContext } from "../types.js";

/** File extensions that aft_outline supports via tree-sitter or markdown parser */
const OUTLINE_EXTENSIONS = new Set([
  ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs",
  ".rs", ".go", ".py", ".rb",
  ".c", ".cpp", ".h", ".hpp", ".cs",
  ".java", ".kt", ".scala",
  ".swift", ".lua", ".ex", ".exs",
  ".hs", ".sol", ".nix",
  ".md", ".mdx",
  ".css", ".html", ".json", ".yaml", ".yml",
  ".sh", ".bash",
]);

const z = tool.schema;

/**
 * Tool definitions for code reading commands: outline + zoom.
 */
export function readingTools(ctx: PluginContext): Record<string, ToolDefinition> {
  return {
    aft_outline: {
      description:
        "Get a structural outline of a source file — lists all top-level symbols with their kind, name, line range, and visibility. Use this to understand file structure before editing. " +
        "Supports single file (via 'file'), multiple files in one call (via 'files' array), or an entire directory (via 'directory').\n" +
        "Each entry includes 'name', 'kind' (function/class/struct/heading/etc), 'range', 'signature', and 'members' (nested children like methods in classes or sub-headings in markdown).\n" +
        "For Markdown files (.md, .mdx): returns heading hierarchy — h1/h2/h3 as nested symbols with section ranges covering all content until the next same-level heading.\n\n" +
        "Parameters:\n" +
        "- file (string, optional): Path to a single file to outline (relative to project root or absolute)\n" +
        "- files (string[], optional): Array of file paths to outline in one call — returns per-file results\n" +
        "- directory (string, optional): Path to a directory — outlines all source files under it recursively\n\n" +
        "Provide either 'file', 'files', or 'directory', not both. Use 'files' to batch multiple outlines in one tool call.",
      args: {
        file: z
          .string()
          .optional()
          .describe(
            "Path to a single source file to outline (relative to project root or absolute)",
          ),
        files: z
          .array(z.string())
          .optional()
          .describe("Array of file paths to outline in one call — returns per-file results"),
        directory: z
          .string()
          .optional()
          .describe("Path to a directory — outlines all source files under it recursively"),
      },
      execute: async (args, context): Promise<string> => {
        const bridge = ctx.pool.getBridge(context.directory);

        // Directory mode: discover source files recursively and batch outline
        if (typeof args.directory === "string") {
          const dirPath = resolve(context.directory, args.directory);
          const files = await discoverSourceFiles(dirPath);
          if (files.length === 0) {
            return JSON.stringify({ success: false, message: `No source files found under ${args.directory}` });
          }
          const response = await bridge.send("outline", { files });
          return JSON.stringify(response);
        }

        if (Array.isArray(args.files) && args.files.length > 0) {
          const response = await bridge.send("outline", { files: args.files });
          return JSON.stringify(response);
        }
        const response = await bridge.send("outline", { file: args.file });
        return JSON.stringify(response);
      },
    },

    aft_zoom: {
      description:
        `Inspect code symbols with call-graph annotations. Returns the full source of named symbols with what they call and what calls them.

Use this when you need to understand a specific function, class, or type in detail — not for reading entire files (use read for that).

**Modes:**

1. **Inspect symbol** — pass filePath + symbol
   Returns full source + call graph annotations.
   Example: { "filePath": "src/app.ts", "symbol": "handleRequest" }

2. **Inspect multiple symbols** — pass filePath + symbols array
   Returns multiple symbols in one call.
   Example: { "filePath": "src/app.ts", "symbols": ["Config", "createApp"] }

3. **Read line range with context** — pass filePath + start_line + end_line
   Returns lines with context_before and context_after.
   Example: { "filePath": "src/app.ts", "start_line": 50, "end_line": 100 }

Parameters:
- filePath (string, required): Path to file
- symbol (string): Name of a single symbol to inspect
- symbols (string[]): Array of symbol names to inspect in one call
- start_line (number): 1-based start line for line-range mode
- end_line (number): 1-based end line for line-range mode (required with start_line)
- context_lines (number): Lines of context around symbols (default: 3)

For Markdown files, use heading text as symbol name (e.g., symbol: "Architecture").`,
      args: {
        filePath: z.string(),
        symbol: z.string().optional(),
        symbols: z.array(z.string()).optional(),
        start_line: z.number().optional(),
        end_line: z.number().optional(),
        context_lines: z.number().optional(),
      },
      execute: async (args, context): Promise<string> => {
        const bridge = ctx.pool.getBridge(context.directory);
        const file = args.filePath as string;

        // Multi-symbol mode: make separate zoom calls and combine results
        if (Array.isArray(args.symbols) && args.symbols.length > 0) {
          const results = [];
          for (const sym of args.symbols as string[]) {
            const params: Record<string, unknown> = { file, symbol: sym };
            if (args.context_lines !== undefined) params.context_lines = args.context_lines;
            const data = await bridge.send("zoom", params);
            results.push(data);
          }
          return JSON.stringify(results);
        }

        // Single symbol or line-range mode
        const params: Record<string, unknown> = { file };
        if (typeof args.symbol === "string") params.symbol = args.symbol;
        if (args.start_line !== undefined) params.start_line = args.start_line;
        if (args.end_line !== undefined) params.end_line = args.end_line;
        if (args.context_lines !== undefined) params.context_lines = args.context_lines;

        const data = await bridge.send("zoom", params);
        return JSON.stringify(data);
      },
    },
  };
}

/** Recursively discover source files under a directory, skipping common noise directories */
const SKIP_DIRS = new Set([
  "node_modules", ".git", "dist", "build", "out", ".next", ".nuxt",
  "target", "__pycache__", ".venv", "venv", "vendor", ".turbo",
  "coverage", ".nyc_output", ".cache",
]);

async function discoverSourceFiles(dir: string, maxFiles = 200): Promise<string[]> {
  const files: string[] = [];

  async function walk(current: string): Promise<void> {
    if (files.length >= maxFiles) return;

    let entries;
    try {
      entries = await readdir(current, { withFileTypes: true });
    } catch {
      return; // permission denied, not a directory, etc.
    }

    for (const entry of entries) {
      if (files.length >= maxFiles) return;

      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name) && !entry.name.startsWith(".")) {
          await walk(join(current, entry.name));
        }
      } else if (entry.isFile()) {
        const ext = extname(entry.name).toLowerCase();
        if (OUTLINE_EXTENSIONS.has(ext)) {
          files.push(join(current, entry.name));
        }
      }
    }
  }

  await walk(dir);
  files.sort();
  return files;
}
