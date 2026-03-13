import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PackageInfo, SearchResult, UnifiedPackage } from "@/types";

export function usePackages() {
  const [installed, setInstalled] = useState<PackageInfo[]>([]);
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(true);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const refreshInstalled = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<PackageInfo[]>("list_packages");
      setInstalled(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  // Load installed packages on mount
  useEffect(() => {
    refreshInstalled();
  }, [refreshInstalled]);

  // Debounced registry search
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);

    if (!query.trim()) {
      setSearchResults([]);
      setSearching(false);
      return;
    }

    setSearching(true);
    debounceRef.current = setTimeout(async () => {
      try {
        const results = await invoke<SearchResult[]>("search_registry", {
          query: query.trim(),
        });
        setSearchResults(results);
      } catch {
        // Search failures are non-fatal — just show installed
        setSearchResults([]);
      } finally {
        setSearching(false);
      }
    }, 300);

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [query]);

  // Build unified list: installed first (filtered by query), then available (not already installed)
  const installedNames = new Set(installed.map((p) => p.name));

  const filteredInstalled: UnifiedPackage[] = installed
    .filter((p) => {
      if (!query) return true;
      const q = query.toLowerCase();
      return (
        p.name.toLowerCase().includes(q) ||
        p.package_type.toLowerCase().includes(q)
      );
    })
    .map((data) => ({ kind: "installed" as const, data }));

  const available: UnifiedPackage[] = query.trim()
    ? searchResults
        .filter((r) => !installedNames.has(r.name))
        .map((data) => ({ kind: "available" as const, data }))
    : [];

  const packages = [...filteredInstalled, ...available];

  return {
    packages,
    installed,
    loading,
    searching,
    error,
    query,
    setQuery,
    refresh: refreshInstalled,
  };
}
