/**
 * Wire types for the MCP change-notification planes, mirroring
 * `swissarmyhammer-plugin::notify`.
 */

/**
 * The kind of data-change op a `notifications/store/changed` notification
 * reports. Mirrors `swissarmyhammer-plugin::notify::ChangeOp` (serialized
 * lowercase).
 */
export type ChangeOp = "created" | "removed" | "updated";
