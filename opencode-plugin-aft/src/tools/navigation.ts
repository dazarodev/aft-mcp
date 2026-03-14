import { tool } from "@opencode-ai/plugin";
import type { ToolDefinition } from "@opencode-ai/plugin";
import type { BinaryBridge } from "../bridge.js";

const z = tool.schema;

/**
 * Tool definitions for navigation commands: configure, call_tree, callers, trace_to, impact, and trace_data.
 */
export function navigationTools(bridge: BinaryBridge): Record<string, ToolDefinition> {
  return {
    aft_configure: {
      description:
        "Configure the AFT binary with the project root directory. Must be called before using call_tree or callers. Sets the worktree scope for call graph analysis.",
      args: {
        project_root: z.string().describe("Absolute path to the project root directory"),
      },
      execute: async (args): Promise<string> => {
        const response = await bridge.send("configure", { project_root: args.project_root });
        return JSON.stringify(response);
      },
    },

    aft_call_tree: {
      description:
        "Get a forward call tree starting from a symbol. Returns a nested tree showing what functions a symbol calls, resolved across files using import chains. Each node includes file path, line number, signature, and whether the edge was resolved. Use after aft_configure.",
      args: {
        file: z.string().describe("Path to the source file containing the symbol (relative to project root or absolute)"),
        symbol: z.string().describe("Name of the symbol to trace calls from"),
        depth: z
          .number()
          .optional()
          .describe("Maximum depth of the call tree (default: 5)"),
      },
      execute: async (args): Promise<string> => {
        const params: Record<string, unknown> = {
          file: args.file,
          symbol: args.symbol,
        };
        if (args.depth !== undefined) params.depth = args.depth;
        const response = await bridge.send("call_tree", params);
        return JSON.stringify(response);
      },
    },

    aft_callers: {
      description:
        "Find all callers of a symbol across the project. Returns call sites grouped by file, showing which functions call the target symbol. Scans all project files and resolves cross-file edges via import chains. Supports recursive depth expansion (callers of callers). Use after aft_configure.",
      args: {
        file: z.string().describe("Path to the source file containing the target symbol (relative to project root or absolute)"),
        symbol: z.string().describe("Name of the symbol to find callers for"),
        depth: z
          .number()
          .optional()
          .describe("Recursive depth: 1 = direct callers only, 2+ = callers of callers (default: 1)"),
      },
      execute: async (args): Promise<string> => {
        const params: Record<string, unknown> = {
          file: args.file,
          symbol: args.symbol,
        };
        if (args.depth !== undefined) params.depth = args.depth;
        const response = await bridge.send("callers", params);
        return JSON.stringify(response);
      },
    },

    aft_trace_to: {
      description:
        "Trace backward from a symbol to all entry points (exported functions, main/init, test functions). Returns complete paths rendered top-down from entry point to target. Use to understand how a deeply-nested function is reached from public API surfaces. Response includes diagnostic fields: total_paths, entry_points_found, max_depth_reached, truncated_paths. Use after aft_configure.",
      args: {
        file: z.string().describe("Path to the source file containing the target symbol (relative to project root or absolute)"),
        symbol: z.string().describe("Name of the symbol to trace to entry points"),
        depth: z
          .number()
          .optional()
          .describe("Maximum backward traversal depth (default: 10)"),
      },
      execute: async (args): Promise<string> => {
        const params: Record<string, unknown> = {
          file: args.file,
          symbol: args.symbol,
        };
        if (args.depth !== undefined) params.depth = args.depth;
        const response = await bridge.send("trace_to", params);
        return JSON.stringify(response);
      },
    },

    aft_impact: {
      description:
        "Analyze the impact of changing a symbol — returns all callers annotated with their signatures, entry point status, source line at call site, and extracted parameter names. Use to understand what breaks when a function signature changes. Response includes diagnostic fields: total_affected, affected_files. Use after aft_configure.",
      args: {
        file: z.string().describe("Path to the source file containing the target symbol (relative to project root or absolute)"),
        symbol: z.string().describe("Name of the symbol to analyze impact for"),
        depth: z
          .number()
          .optional()
          .describe("Maximum transitive caller depth (default: 5)"),
      },
      execute: async (args): Promise<string> => {
        const params: Record<string, unknown> = {
          file: args.file,
          symbol: args.symbol,
        };
        if (args.depth !== undefined) params.depth = args.depth;
        const response = await bridge.send("impact", params);
        return JSON.stringify(response);
      },
    },

    aft_trace_data: {
      description:
        "Trace how an expression flows through variable assignments and function parameters across files. Tracks variable renames (const x = expr → x is the new tracking name), cross-file argument-to-parameter matching (f(x) → parameter 'input' in f's definition), and flags approximations where tracking is uncertain (destructuring, spread, unresolved calls). Response includes diagnostic fields: depth_limited, per-hop approximate flag. Use after aft_configure.",
      args: {
        file: z.string().describe("Path to the source file containing the symbol (relative to project root or absolute)"),
        symbol: z.string().describe("Name of the function containing the expression to trace"),
        expression: z.string().describe("The expression or variable name to track through data flow"),
        depth: z
          .number()
          .optional()
          .describe("Maximum cross-file hop depth (default: 5)"),
      },
      execute: async (args): Promise<string> => {
        const params: Record<string, unknown> = {
          file: args.file,
          symbol: args.symbol,
          expression: args.expression,
        };
        if (args.depth !== undefined) params.depth = args.depth;
        const response = await bridge.send("trace_data", params);
        return JSON.stringify(response);
      },
    },
  };
}
