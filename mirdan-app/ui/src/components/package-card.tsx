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
import type { UnifiedPackage } from "@/types";
import { cn } from "@/lib/utils";
import {
  FolderOpen,
  ExternalLink,
  Trash2,
  RefreshCw,
  Loader2,
  Download,
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
  pkg: UnifiedPackage;
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

  const isInstalled = pkg.kind === "installed";
  const name = pkg.data.name;
  const packageType = pkg.data.package_type;

  // Extract fields with proper type narrowing
  const description =
    pkg.kind === "available" ? pkg.data.description : undefined;
  const author = pkg.kind === "available" ? pkg.data.author : undefined;
  const downloads = pkg.kind === "available" ? pkg.data.downloads : 0;
  const version = pkg.kind === "installed" ? pkg.data.version : undefined;
  const targets = pkg.kind === "installed" ? pkg.data.targets : [];

  async function handleInstall() {
    setBusy("install");
    try {
      const spec =
        pkg.kind === "available" ? pkg.data.qualified_name : name;
      const msg = await invoke<string>("install_package", { spec });
      toast(msg, "success");
      onRefresh();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setBusy(null);
    }
  }

  async function handleShowInFinder() {
    if (pkg.kind !== "installed") return;
    const path = await invoke<string | null>("get_package_path", { name });
    if (path) {
      await invoke("open_external", { target: path });
    } else {
      toast("Could not find package directory", "error");
    }
  }

  async function handleOpenRegistry() {
    const url = await invoke<string>("get_registry_url", { name });
    await invoke("open_external", { target: url });
  }

  async function handleUninstall() {
    if (pkg.kind !== "installed") return;
    setBusy("uninstall");
    try {
      const msg = await invoke<string>("uninstall_package", {
        spec: pkg.data.source,
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
    if (pkg.kind !== "installed") return;
    setBusy("update");
    try {
      const msg = await invoke<string>("update_package", {
        spec: pkg.data.source,
      });
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
      {/* Row 1: name, badge, version, actions */}
      <div className="flex items-center gap-2">
        <span className="font-medium text-sm">{name}</span>
        <Badge
          variant="outline"
          className={cn(
            "text-[10px] px-1.5 py-0 h-4 font-normal border-0",
            typeColors[packageType] ?? ""
          )}
        >
          {packageType}
        </Badge>

        {version && (
          <span className="text-xs text-muted-foreground">
            {version}
          </span>
        )}

        {/* Action buttons — right-aligned, visible on hover or selected */}
        <div
          className={cn(
            "flex gap-0.5 shrink-0 ml-auto opacity-0 transition-opacity",
            "group-hover:opacity-100",
            selected && "opacity-100"
          )}
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          {isInstalled ? (
            <>
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
            </>
          ) : (
            <>
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
                    className="h-7 w-7 text-primary hover:text-primary"
                    onClick={handleInstall}
                    disabled={busy !== null}
                  >
                    {busy === "install" ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                    ) : (
                      <Download className="w-3.5 h-3.5" />
                    )}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Install</TooltipContent>
              </Tooltip>
            </>
          )}
        </div>
      </div>

      {/* Row 2: description or targets — full width */}
      {description && (
        <div className="text-xs text-muted-foreground mt-1">
          {description}
        </div>
      )}
      {targets.length > 0 && (
        <div className="text-xs text-muted-foreground mt-1">
          {targets.join(", ")}
        </div>
      )}

      {/* Row 3: author + downloads for available packages */}
      {author && (
        <div className="text-xs text-muted-foreground/70 mt-0.5">
          by {author}
          {downloads > 0 && ` · ${downloads.toLocaleString()} downloads`}
        </div>
      )}
    </div>
  );
}
