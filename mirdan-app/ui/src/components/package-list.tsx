import { useState, useMemo } from "react";
import { usePackages } from "@/lib/use-packages";
import { PackageCard } from "@/components/package-card";
import { Search, Package, Loader2 } from "lucide-react";

export function PackageList() {
  const { packages, loading, error, refresh } = usePackages();
  const [filter, setFilter] = useState("");
  const [selectedName, setSelectedName] = useState<string | null>(null);

  const filtered = useMemo(() => {
    if (!filter) return packages;
    const q = filter.toLowerCase();
    return packages.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        p.package_type.toLowerCase().includes(q)
    );
  }, [packages, filter]);

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

  return (
    <div className="h-full flex flex-col">
      {/* Search bar */}
      <div className="px-3 py-2 border-b border-border">
        <div className="relative">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <input
            type="text"
            placeholder="Filter packages..."
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            className="w-full pl-9 pr-3 py-2 text-sm bg-background border border-input rounded-md focus:outline-none focus:ring-1 focus:ring-ring"
          />
        </div>
      </div>

      {/* Package list */}
      <div className="flex-1 overflow-y-auto">
        {filtered.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground gap-3 p-8">
            <Package className="w-10 h-10" />
            <p className="text-sm">
              {packages.length === 0
                ? "No packages installed"
                : "No packages match your filter"}
            </p>
            {packages.length === 0 && (
              <a
                href="https://mirdan.ai"
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-primary hover:underline"
              >
                Browse packages on mirdan.ai
              </a>
            )}
          </div>
        ) : (
          filtered.map((pkg) => (
            <PackageCard
              key={pkg.name}
              pkg={pkg}
              selected={selectedName === pkg.name}
              onSelect={() =>
                setSelectedName(
                  selectedName === pkg.name ? null : pkg.name
                )
              }
              onRefresh={refresh}
            />
          ))
        )}
      </div>

      {/* Status bar */}
      <div className="px-4 py-1.5 border-t border-border text-xs text-muted-foreground">
        {filtered.length} package{filtered.length !== 1 ? "s" : ""}
        {filter && ` (${packages.length} total)`}
      </div>
    </div>
  );
}
