/** Mirrors the Rust PackageInfo struct from commands.rs */
export interface PackageInfo {
  name: string;
  package_type: string;
  version: string;
  targets: string[];
  store_path: string | null;
}
