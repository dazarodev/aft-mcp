import * as path from "node:path";
import type { ToolContext } from "@opencode-ai/plugin";

export function resolveAbsolutePath(context: ToolContext, target: string): string {
  return path.isAbsolute(target) ? target : path.resolve(context.directory, target);
}

export function resolveRelativePattern(context: ToolContext, target: string): string {
  return path.relative(context.worktree, resolveAbsolutePath(context, target)) || ".";
}

export function resolveRelativePatterns(context: ToolContext, targets: string[]): string[] {
  const seen = new Set<string>();
  const patterns: string[] = [];

  for (const target of targets) {
    if (!target) continue;
    const pattern = resolveRelativePattern(context, target);
    if (seen.has(pattern)) continue;
    seen.add(pattern);
    patterns.push(pattern);
  }

  return patterns;
}

export function workspacePattern(_context: ToolContext): string {
  return ".";
}

export async function askEditPermission(
  context: ToolContext,
  patterns: string[],
  metadata: Record<string, unknown> = {},
): Promise<string | undefined> {
  try {
    await context.ask({
      permission: "edit",
      patterns: patterns.length > 0 ? patterns : [workspacePattern(context)],
      always: ["*"],
      metadata,
    });
    return undefined;
  } catch (error) {
    if (error instanceof Error && error.message) {
      return error.message;
    }
    return "Permission denied.";
  }
}

export function permissionDeniedResponse(message: string): string {
  return JSON.stringify({
    success: false,
    code: "permission_denied",
    message,
    error: message,
  });
}
