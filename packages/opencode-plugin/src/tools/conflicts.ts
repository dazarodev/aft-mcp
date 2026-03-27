import type { ToolDefinition } from "@opencode-ai/plugin";
import type { PluginContext } from "../types.js";

/**
 * Tool definition for the git conflict discovery and parsing tool.
 */
export function conflictTools(ctx: PluginContext): Record<string, ToolDefinition> {
  return {
    aft_conflicts: {
      description:
        "Show all git merge conflicts across the repository — returns line-numbered conflict regions with context for every conflicted file in a single call.",
      args: {},
      execute: async (_args, context): Promise<string> => {
        const bridge = ctx.pool.getBridge(context.directory, context.sessionID);
        const response = await bridge.send("git_conflicts", {});
        if (response.success === false) {
          throw new Error((response.message as string) || "git_conflicts failed");
        }
        return response.text as string;
      },
    },
  };
}
