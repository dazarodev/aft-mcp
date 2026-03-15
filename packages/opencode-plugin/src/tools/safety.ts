import type { ToolDefinition } from "@opencode-ai/plugin";
import { tool } from "@opencode-ai/plugin";
import type { PluginContext } from "../types.js";

const z = tool.schema;

/**
 * Tool definitions for safety & recovery commands: undo, edit_history,
 * checkpoint, restore_checkpoint, list_checkpoints.
 */
export function safetyTools(ctx: PluginContext): Record<string, ToolDefinition> {
  return {
    aft_safety: {
      description:
        "File safety and recovery operations.\n" +
        "Ops:\n" +
        "- 'undo': Undo the last edit to a file. Needs 'file'.\n" +
        "- 'history': List all edit snapshots for a file. Needs 'file'.\n" +
        "- 'checkpoint': Save a named snapshot of tracked files. Needs 'name', optional 'files' array.\n" +
        "- 'restore': Restore files to a checkpoint state. Needs 'name'.\n" +
        "- 'list': List all available checkpoints.\n" +
        "Use checkpoint before risky multi-file changes. Use undo for quick single-file rollback.\n" +
        "Note: backups are in-memory (lost on restart). Per-file undo stack is capped at 20 entries (oldest evicted).",
      args: {
        op: z
          .enum(["undo", "history", "checkpoint", "restore", "list"])
          .describe("Safety operation"),
        file: z.string().optional().describe("File path (required for undo, history)"),
        name: z.string().optional().describe("Checkpoint name (required for checkpoint, restore)"),
        files: z
          .array(z.string())
          .optional()
          .describe(
            "Specific files to include in checkpoint (optional, defaults to all tracked files)",
          ),
      },
      execute: async (args, context): Promise<string> => {
        const bridge = ctx.pool.getBridge(context.directory);
        const op = args.op as string;
        const commandMap: Record<string, string> = {
          undo: "undo",
          history: "edit_history",
          checkpoint: "checkpoint",
          restore: "restore_checkpoint",
          list: "list_checkpoints",
        };
        const params: Record<string, unknown> = {};
        if (args.file !== undefined) params.file = args.file;
        if (args.name !== undefined) params.name = args.name;
        if (args.files !== undefined) params.files = args.files;
        const response = await bridge.send(commandMap[op], params);
        return JSON.stringify(response);
      },
    },
  };
}
