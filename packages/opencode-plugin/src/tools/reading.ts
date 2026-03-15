import type { ToolDefinition } from "@opencode-ai/plugin";
import { tool } from "@opencode-ai/plugin";
import { queryLspHints } from "../lsp.js";
import type { PluginContext } from "../types.js";

const z = tool.schema;

/**
 * Tool definitions for code reading commands: outline and zoom.
 */
export function readingTools(ctx: PluginContext): Record<string, ToolDefinition> {
  return {
    aft_outline: {
      description:
        "Get a structural outline of a source file — lists all top-level symbols with their kind, name, line range, and visibility. Use this to understand file structure before editing. " +
        "Supports single file (via 'file') or multiple files in one call (via 'files' array).\n" +
        "Each entry includes 'name', 'kind' (function/class/struct/heading/etc), 'range', 'signature', and 'members' (nested children like methods in classes or sub-headings in markdown).\n" +
        "For Markdown files (.md, .mdx): returns heading hierarchy — h1/h2/h3 as nested symbols with section ranges covering all content until the next same-level heading.",
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
      },
      execute: async (args, context): Promise<string> => {
        const bridge = ctx.pool.getBridge(context.directory);
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
        "Deep-inspect symbols or read arbitrary line ranges from a source file.\n" +
        "Returns full source, surrounding context lines, and call-graph annotations (calls_out, called_by) for code symbols.\n" +
        "Supports three access patterns:\n" +
        "- 'symbol': Inspect a named symbol (function, class, type, or markdown heading)\n" +
        "- 'symbols': Inspect multiple symbols in one call — returns an array of results\n" +
        "- 'start_line' + 'end_line': Read arbitrary line range (1-based) without needing a symbol name\n" +
        "For Markdown: use heading text as symbol name (e.g. symbol='Architecture') to read entire section.",
      args: {
        file: z.string().describe("Path to the source file containing the symbol"),
        symbol: z.string().optional().describe("Name of a single symbol to inspect"),
        symbols: z
          .array(z.string())
          .optional()
          .describe("Array of symbol names to inspect in one call — returns results for each"),
        context_lines: z
          .number()
          .optional()
          .describe(
            "Number of lines of context to include above and below the symbol (default: 3)",
          ),
        scope: z
          .string()
          .optional()
          .describe(
            "Qualified scope to disambiguate symbols with the same name (e.g. 'ClassName.method')",
          ),
        start_line: z.number().optional().describe("Start line (1-based) for line-range read mode"),
        end_line: z
          .number()
          .optional()
          .describe("End line (1-based, inclusive) for line-range read mode"),
      },
      execute: async (args, context): Promise<string> => {
        const bridge = ctx.pool.getBridge(context.directory);

        // Batch symbols mode: zoom into multiple symbols in one call
        if (Array.isArray(args.symbols) && args.symbols.length > 0) {
          const results = await Promise.all(
            (args.symbols as string[]).map(async (sym) => {
              const params: Record<string, unknown> = { file: args.file, symbol: sym };
              if (args.context_lines !== undefined)
                params.context_lines = Number(args.context_lines);
              if (args.scope !== undefined) params.scope = args.scope;
              const hints = await queryLspHints(ctx.client, sym);
              if (hints) params.lsp_hints = hints;
              return bridge.send("zoom", params);
            }),
          );
          return JSON.stringify(results);
        }

        // Single symbol or line-range mode
        const params: Record<string, unknown> = { file: args.file };
        if (args.symbol !== undefined) params.symbol = args.symbol;
        if (args.context_lines !== undefined) params.context_lines = Number(args.context_lines);
        if (args.scope !== undefined) params.scope = args.scope;
        if (args.start_line !== undefined) params.start_line = Number(args.start_line);
        if (args.end_line !== undefined) params.end_line = Number(args.end_line);

        if (args.symbol) {
          const hints = await queryLspHints(ctx.client, args.symbol as string);
          if (hints) params.lsp_hints = hints;
        }

        const response = await bridge.send("zoom", params);
        return JSON.stringify(response);
      },
    },
  };
}
