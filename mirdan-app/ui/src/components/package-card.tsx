import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { toast } from "@/components/toast";
import type { PackageInfo } from "@/types";
import { cn } from "@/lib/utils";
import {
  FolderOpen,
  ExternalLink,
  Trash2,
  RefreshCw,
  Loader2,
} from "lucide-react";

const typeColors: Record<string, string> = {
  skill: "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200",
  tool: "bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200",
  validator:
    "bg-amber-100 text-amber-800 dark:bg-amber-900 dark:text-amber-200",
  plugin:
    "bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-200",
};

interface PackageCardProps {
  pkg: PackageInfo;
  selected: boolean;
  onSelect: () => void;
  onRefresh: () => void;
}

export function PackageCard({
  pkg,
  selected,
  onSelect,
  onRefresh,
}: PackageCardProps) {
  const [busy, setBusy] = useState<string | null>(null);

  async function handleShowInFinder() {
    const path = await invoke<string | null>("get_package_path", {
      name: pkg.name,
    });
    if (path) {
      await invoke("open_external", { target: path });
    } else {
      toast("Could not find package directory", "error");
    }
  }

  async function handleOpenRegistry() {
    const url = await invoke<string>("get_registry_url", { name: pkg.name });
    await invoke("open_external", { target: url });
  }

  async function handleUninstall() {
    if (!window.confirm(`Uninstall "${pkg.name}"?`)) return;
    setBusy("uninstall");
    try {
      const msg = await invoke<string>("uninstall_package", {
        spec: pkg.name,
      });
      toast(msg, "success");
      onRefresh();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setBusy(null);
    }
  }

  async function handleUpdate() {
    setBusy("update");
    try {
      const msg = await invoke<string>("update_package", { spec: pkg.name });
      toast(msg, "success");
      onRefresh();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setBusy(null);
    }
  }

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => e.key === "Enter" && onSelect()}
      className={cn(
        "group w-full text-left px-4 py-3 border-b border-border transition-colors cursor-pointer",
        "hover:bg-accent/50",
        selected && "bg-accent"
      )}
    >
      <div className="flex items-center gap-2">
        <span className="font-medium text-sm">{pkg.name}</span>
        <Badge
          variant="outline"
          className={cn(
            "text-[10px] px-1.5 py-0 h-4 font-normal border-0",
            typeColors[pkg.package_type] ?? ""
          )}
        >
          {pkg.package_type}
        </Badge>
        <span className="text-xs text-muted-foreground ml-auto mr-2">
          {pkg.version}
        </span>

        {/* Action buttons — visible on hover or when selected */}
        <div
          className={cn(
            "flex gap-0.5 opacity-0 transition-opacity",
            "group-hover:opacity-100",
            selected && "opacity-100"
          )}
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={handleShowInFinder}
              >
                <FolderOpen className="w-3.5 h-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Show in Finder</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={handleOpenRegistry}
              >
                <ExternalLink className="w-3.5 h-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Open on mirdan.ai</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={handleUpdate}
                disabled={busy !== null}
              >
                {busy === "update" ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <RefreshCw className="w-3.5 h-3.5" />
                )}
              </Button>
            </TooltipTrigger>
            <TooltipContent>Update</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7 text-destructive hover:text-destructive"
                onClick={handleUninstall}
                disabled={busy !== null}
              >
                {busy === "uninstall" ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Trash2 className="w-3.5 h-3.5" />
                )}
              </Button>
            </TooltipTrigger>
            <TooltipContent>Uninstall</TooltipContent>
          </Tooltip>
        </div>
      </div>
      {pkg.targets.length > 0 && (
        <div className="text-xs text-muted-foreground mt-1">
          {pkg.targets.join(", ")}
        </div>
      )}
    </div>
  );
}
