import { tool } from "@opencode-ai/plugin";
import type { ToolDefinition } from "@opencode-ai/plugin";
import type { BinaryBridge } from "../bridge.js";

const z = tool.schema;

/**
 * Tool definitions for refactoring commands: move_symbol, extract_function, inline_symbol.
 */
export function refactoringTools(bridge: BinaryBridge): Record<string, ToolDefinition> {
  return {
    aft_move_symbol: {
      description:
        "Move a top-level symbol (function, class, type, const, etc.) from one file to another. Updates all import statements across the workspace so consumers point to the new location. Supports dry_run mode to preview changes without writing. Creates a checkpoint before mutations for rollback safety. Requires aft_configure to be called first.",
      args: {
        file: z.string().describe("Path to the source file containing the symbol (relative to project root or absolute)"),
        symbol: z.string().describe("Name of the top-level symbol to move"),
        destination: z.string().describe("Path to the destination file (will be created if it doesn't exist)"),
        scope: z
          .string()
          .optional()
          .describe("Disambiguation scope when multiple symbols share the same name"),
        dry_run: z
          .boolean()
          .optional()
          .describe("If true, returns a preview diff without modifying files on disk"),
      },
      execute: async (args): Promise<string> => {
        const params: Record<string, unknown> = {
          file: args.file,
          symbol: args.symbol,
          destination: args.destination,
        };
        if (args.scope !== undefined) params.scope = args.scope;
        if (args.dry_run !== undefined) params.dry_run = args.dry_run;
        const response = await bridge.send("move_symbol", params);
        return JSON.stringify(response);
      },
    },

    aft_extract_function: {
      description:
        "Extract a range of code lines into a new function with auto-detected parameters and return value. Analyses free variables in the selected range to build the parameter list. Supports TS/JS/TSX and Python. Use dry_run mode to preview the extraction diff without writing. Returns parameters, return_type, and syntax_valid. Error codes: unsupported_language, this_reference_in_range.",
      args: {
        file: z.string().describe("Path to the file containing the code to extract (relative to project root or absolute)"),
        name: z.string().describe("Name for the new extracted function"),
        start_line: z.number().describe("First line of the range to extract (0-indexed)"),
        end_line: z.number().describe("Last line of the range to extract (exclusive, 0-indexed)"),
        dry_run: z
          .boolean()
          .optional()
          .describe("If true, returns a preview diff without modifying files on disk"),
      },
      execute: async (args): Promise<string> => {
        const params: Record<string, unknown> = {
          file: args.file,
          name: args.name,
          start_line: args.start_line,
          end_line: args.end_line,
        };
        if (args.dry_run !== undefined) params.dry_run = args.dry_run;
        const response = await bridge.send("extract_function", params);
        return JSON.stringify(response);
      },
    },

    aft_inline_symbol: {
      description:
        "Replace a function call with the function's body, substituting arguments for parameters. Validates single-return constraint and detects scope conflicts at the call site. Supports TS/JS/TSX and Python. Use dry_run mode to preview the inline diff without writing. Returns call_context, substitutions, and conflicts. Error codes: multiple_returns (with return_count), scope_conflict (with conflicting_names and suggestions), symbol_not_found, call_not_found.",
      args: {
        file: z.string().describe("Path to the file containing both the function and its call site (relative to project root or absolute)"),
        symbol: z.string().describe("Name of the function to inline"),
        call_site_line: z.number().describe("Line number where the call expression is located (0-indexed)"),
        dry_run: z
          .boolean()
          .optional()
          .describe("If true, returns a preview diff without modifying files on disk"),
      },
      execute: async (args): Promise<string> => {
        const params: Record<string, unknown> = {
          file: args.file,
          symbol: args.symbol,
          call_site_line: args.call_site_line,
        };
        if (args.dry_run !== undefined) params.dry_run = args.dry_run;
        const response = await bridge.send("inline_symbol", params);
        return JSON.stringify(response);
      },
    },
  };
}
