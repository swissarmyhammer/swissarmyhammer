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
