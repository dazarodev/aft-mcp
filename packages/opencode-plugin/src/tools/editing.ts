import type { ToolDefinition } from "@opencode-ai/plugin";
import { tool } from "@opencode-ai/plugin";
import { queryLspHints } from "../lsp.js";
import type { PluginContext } from "../types.js";

const z = tool.schema;

/** Valid operations for edit_symbol. */
const editOperationEnum = z
  .enum(["replace", "delete", "insert_before", "insert_after"])
  .describe("The edit operation to perform on the symbol");

/** Schema for a single batch edit item — either match-replace or line-range. */
const batchEditItem = z.union([
  z.object({
    match: z.string().describe("Text pattern to find and replace"),
    replacement: z.string().describe("Replacement text"),
  }),
  z.object({
    line_start: z.number().describe("Start line number (1-indexed)"),
    line_end: z.number().describe("End line number (1-indexed, inclusive)"),
    content: z.string().describe("Content to replace the line range with"),
  }),
]);

/**
 * Tool definitions for code editing commands: write, edit_symbol, edit_match, batch.
 */
export function editingTools(ctx: PluginContext): Record<string, ToolDefinition> {
  return {
    aft_edit: {
      description:
        "Edit files with tree-sitter precision. All modes auto-backup before changes and support dry_run.\n" +
        "Modes:\n" +
        "- 'symbol': Edit a named symbol (function, class, type) — preferred for code changes. Needs 'symbol', 'operation' (replace/delete/insert_before/insert_after), and 'content'. Response includes context_before/context_after (3 lines each) to detect duplication.\n" +
        "- 'match': Find and replace text by content match — use for config values, strings, unnamed code. Needs 'match', 'replacement'. Set replace_all=true to replace ALL occurrences at once. Returns ambiguous_match if multiple hits without occurrence or replace_all. For multi-line matching, use actual newlines in the JSON string value. Supports glob patterns in 'file' (e.g. '**/*.ts') to replace across multiple files at once — returns {files: [...], total_replacements, total_files}.\n" +
        "- 'write': Write full file content — for new files or complete rewrites. Needs 'content'.\n" +
        "- 'batch': Multiple edits in one file atomically — each edit is {match, replacement} or {line_start, line_end, content} (1-based, inclusive). Supports per-edit 'occurrence' for disambiguation. Set content to empty string to delete lines entirely. line_start == total_lines+1 appends at EOF. line_end is auto-clamped to last line (safe to overshoot).\n" +
        "- 'transaction': Atomic multi-file edits with rollback — if any file fails, all revert. Needs 'operations' array of {file, command, ...}.\n" +
        "Returns formatted, validation_errors, backup_id.",
      args: {
        mode: z.enum(["symbol", "match", "write", "batch", "transaction"]).describe("Editing mode"),
        file: z
          .string()
          .optional()
          .describe("Path to the file (required for all modes except transaction)"),
        // symbol mode
        symbol: z.string().optional().describe("Symbol name to edit (symbol mode)"),
        operation: editOperationEnum.optional().describe("Edit operation (symbol mode)"),
        scope: z
          .string()
          .optional()
          .describe("Qualified scope for disambiguation (e.g. 'ClassName.method')"),
        // match mode
        match: z.string().optional().describe("Text to find (match mode)"),
        replacement: z.string().optional().describe("Replacement text (match mode)"),
        occurrence: z
          .number()
          .describe(
            "Zero-based index selecting which occurrence to replace when multiple matches exist (0 = first, 1 = second, etc). Use with ambiguous_match response. (match mode)",
          ),
        replace_all: z
          .boolean()
          .optional()
          .describe(
            "Replace ALL occurrences instead of disambiguating (match mode, default: false)",
          ),
        // write + symbol content
        content: z
          .string()
          .optional()
          .describe("New content (write mode: full file, symbol mode: replacement body)"),
        create_dirs: z
          .boolean()
          .optional()
          .describe("Create parent directories (write mode, default: false)"),
        // batch mode
        edits: z
          .array(batchEditItem)
          .optional()
          .describe("Array of edits to apply atomically (batch mode)"),
        // transaction mode
        operations: z
          .array(
            z.object({
              file: z.string().describe("File path"),
              command: z.enum(["write", "edit_match"]).describe("Operation type"),
              content: z.string().optional().describe("Full content for write"),
              match: z.string().optional().describe("Text to find for edit_match"),
              replacement: z.string().optional().describe("Replacement for edit_match"),
            }),
          )
          .optional()
          .describe("Array of file operations (transaction mode)"),
        // common
        validate: z
          .enum(["syntax", "full"])
          .optional()
          .describe("Validation level: 'syntax' (default) or 'full'"),
        dry_run: z.boolean().optional().describe("Preview as unified diff without modifying files"),
      },
      execute: async (args, context): Promise<string> => {
        const bridge = ctx.pool.getBridge(context.directory);
        const mode = args.mode as string;
        const params: Record<string, unknown> = {};

        if (args.file !== undefined) params.file = args.file;
        if (args.validate !== undefined) params.validate = args.validate;
        if (args.dry_run !== undefined) params.dry_run = args.dry_run;

        let command: string;

        switch (mode) {
          case "symbol": {
            command = "edit_symbol";
            params.symbol = args.symbol;
            params.operation = args.operation;
            if (args.content !== undefined) params.content = args.content;
            if (args.scope !== undefined) params.scope = args.scope;
            const hints = await queryLspHints(ctx.client, args.symbol as string);
            if (hints) params.lsp_hints = hints;
            break;
          }
          case "match": {
            command = "edit_match";
            params.match = args.match;
            params.replacement = args.replacement;
            if (args.occurrence !== undefined) params.occurrence = Number(args.occurrence);
            if (args.replace_all !== undefined) params.replace_all = args.replace_all;
            break;
          }
          case "write": {
            command = "write";
            params.content = args.content;
            if (args.create_dirs !== undefined) params.create_dirs = args.create_dirs;
            break;
          }
          case "batch": {
            command = "batch";
            params.edits = args.edits;
            break;
          }
          case "transaction": {
            command = "transaction";
            params.operations = args.operations;
            break;
          }
          default:
            command = mode;
        }

        const response = await bridge.send(command, params);
        return JSON.stringify(response);
      },
    },
  };
}
