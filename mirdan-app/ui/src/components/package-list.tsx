import { useState } from "react";
import { usePackages } from "@/lib/use-packages";
import { PackageCard } from "@/components/package-card";
import { Search, Package, Loader2 } from "lucide-react";

export function PackageList() {
  const { packages, loading, searching, error, query, setQuery, refresh } =
    usePackages();
  const [selectedName, setSelectedName] = useState<string | null>(null);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground">
        <Loader2 className="w-5 h-5 animate-spin mr-2" />
        Loading packages...
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full text-destructive">
        Error: {error}
      </div>
    );
  }

  const installedCount = packages.filter((p) => p.kind === "installed").length;
  const availableCount = packages.filter((p) => p.kind === "available").length;

  return (
    <div className="h-full flex flex-col">
      {/* Search bar */}
      <div className="px-3 py-2 border-b border-border">
        <div className="relative">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search packages..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="w-full pl-9 pr-3 py-2 text-sm bg-background border border-input rounded-md focus:outline-none focus:ring-1 focus:ring-ring"
          />
          {searching && (
            <Loader2 className="absolute right-2.5 top-2.5 h-4 w-4 animate-spin text-muted-foreground" />
          )}
        </div>
      </div>

      {/* Package list */}
      <div className="flex-1 overflow-y-auto">
        {packages.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground gap-3 p-8">
            <Package className="w-10 h-10" />
            <p className="text-sm">
              {query
                ? "No packages match your search"
                : "No packages installed"}
            </p>
            {!query && (
              <p className="text-xs">
                Type to search the mirdan registry
              </p>
            )}
          </div>
        ) : (
          packages.map((pkg) => {
            const name =
              pkg.kind === "installed" ? pkg.data.name : pkg.data.name;
            return (
              <PackageCard
                key={`${pkg.kind}-${name}`}
                pkg={pkg}
                selected={selectedName === name}
                onSelect={() =>
                  setSelectedName(selectedName === name ? null : name)
                }
                onRefresh={refresh}
              />
            );
          })
        )}
      </div>

      {/* Status bar */}
      <div className="px-4 py-1.5 border-t border-border text-xs text-muted-foreground">
        {installedCount} installed
        {availableCount > 0 && ` · ${availableCount} available`}
      </div>
    </div>
  );
}
