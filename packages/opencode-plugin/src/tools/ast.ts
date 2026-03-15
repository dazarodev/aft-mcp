/**
 * Tool definitions for AST pattern search and replace using ast-grep.
 * Supports meta-variables ($VAR for single node, $$$ for multiple nodes).
 * Patterns must be complete AST nodes (valid code fragments).
 */

import { tool } from "@opencode-ai/plugin";

const z = tool.schema;

import type { ToolDefinition } from "@opencode-ai/plugin";
import type { PluginContext } from "../types.js";

const SUPPORTED_LANGS = [
  "bash",
  "c",
  "cpp",
  "csharp",
  "css",
  "elixir",
  "go",
  "haskell",
  "html",
  "java",
  "javascript",
  "json",
  "kotlin",
  "lua",
  "nix",
  "php",
  "python",
  "ruby",
  "rust",
  "scala",
  "solidity",
  "swift",
  "typescript",
  "tsx",
  "yaml",
] as const;

export function astTools(ctx: PluginContext): Record<string, ToolDefinition> {
  return {
    aft_ast_search: {
      description:
        "Search code patterns across filesystem using AST-aware matching. Supports 25 languages. " +
        "Use meta-variables: $VAR (single node), $$$ (multiple nodes). " +
        "IMPORTANT: Patterns must be complete AST nodes (valid code). " +
        "For functions, include params and body: 'export async function $NAME($$$) { $$$ }' not 'export async function $NAME'. " +
        "Examples: 'console.log($MSG)', 'def $FUNC($$$):', 'async function $NAME($$$)'",
      args: {
        pattern: z
          .string()
          .describe("AST pattern with meta-variables ($VAR, $$$). Must be complete AST node."),
        lang: z.enum(SUPPORTED_LANGS).describe("Target language"),
        paths: z.array(z.string()).optional().describe("Paths to search (default: ['.'])"),
        globs: z
          .array(z.string())
          .optional()
          .describe("Include/exclude globs (prefix ! to exclude)"),
        context: z.number().optional().describe("Context lines around match"),
      },
      execute: async (args, context): Promise<string> => {
        const bridge = ctx.pool.getBridge(context.directory);
        const params: Record<string, unknown> = {
          pattern: args.pattern,
          lang: args.lang,
        };
        if (args.paths) params.paths = args.paths;
        if (args.globs) params.globs = args.globs;
        if (args.context !== undefined) params.context = Number(args.context);
        const response = await bridge.send("ast_search", params);
        return JSON.stringify(response);
      },
    },

    aft_ast_replace: {
      description:
        "Replace code patterns across filesystem with AST-aware rewriting. Dry-run by default. " +
        "Use meta-variables in rewrite to preserve matched content. " +
        "Example: pattern='console.log($MSG)' rewrite='logger.info($MSG)'",
      args: {
        pattern: z.string().describe("AST pattern to match"),
        rewrite: z.string().describe("Replacement pattern (can use $VAR from pattern)"),
        lang: z.enum(SUPPORTED_LANGS).describe("Target language"),
        paths: z.array(z.string()).optional().describe("Paths to search"),
        globs: z.array(z.string()).optional().describe("Include/exclude globs"),
        dryRun: z.boolean().optional().describe("Preview changes without applying (default: true)"),
      },
      execute: async (args, context): Promise<string> => {
        const bridge = ctx.pool.getBridge(context.directory);
        const params: Record<string, unknown> = {
          pattern: args.pattern,
          rewrite: args.rewrite,
          lang: args.lang,
        };
        if (args.paths) params.paths = args.paths;
        if (args.globs) params.globs = args.globs;
        // Default to dry_run=true for safety
        params.dry_run = args.dryRun !== false;
        const response = await bridge.send("ast_replace", params);
        return JSON.stringify(response);
      },
    },
  };
}
