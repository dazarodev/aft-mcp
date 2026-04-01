import type { PluginInput } from "@opencode-ai/plugin";
import type { AftConfig } from "./config.js";
import type { BridgePool } from "./pool.js";

/**
 * Shared context passed to all tool factory functions.
 * Bundles the binary bridge, the OpenCode SDK client, and plugin config.
 */
export interface PluginContext {
  pool: BridgePool;
  client: PluginInput["client"];
  config: AftConfig;
}
