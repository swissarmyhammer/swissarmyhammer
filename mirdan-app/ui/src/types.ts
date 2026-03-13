/** Mirrors the Rust PackageInfo struct from commands.rs */
export interface PackageInfo {
  name: string;
  /** Lockfile key / source URL — use for uninstall and update operations. */
  source: string;
  package_type: string;
  version: string;
  targets: string[];
  store_path: string | null;
}

/** A registry search result from commands.rs */
export interface SearchResult {
  name: string;
  /** Qualified name for install routing (e.g. "owner/repo/skill"). */
  qualified_name: string;
  description: string;
  author: string;
  package_type: string;
  downloads: number;
}

/** Unified item for the package list — either installed or available from registry */
export type UnifiedPackage =
  | { kind: "installed"; data: PackageInfo }
  | { kind: "available"; data: SearchResult };
