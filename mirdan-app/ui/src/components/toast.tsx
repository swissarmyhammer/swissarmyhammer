import { useState, useEffect, useCallback } from "react";
import { cn } from "@/lib/utils";
import { X } from "lucide-react";

export interface Toast {
  id: number;
  message: string;
  variant: "success" | "error";
}

// Module-level state: acceptable for a single-page Tauri tray app.
// nextId is monotonic and never resets, so IDs aren't unique across
// hot-reloads — harmless since old toasts are already dismissed.
let nextId = 0;
let addToastFn: ((message: string, variant: "success" | "error") => void) | null = null;

/** Show a toast from anywhere. Call after ToastContainer is mounted. */
export function toast(message: string, variant: "success" | "error" = "success") {
  addToastFn?.(message, variant);
}

export function ToastContainer() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const addToast = useCallback((message: string, variant: "success" | "error") => {
    const id = nextId++;
    setToasts((prev) => [...prev, { id, message, variant }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4000);
  }, []);

  useEffect(() => {
    addToastFn = addToast;
    return () => { addToastFn = null; };
  }, [addToast]);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={cn(
            "flex items-center gap-2 px-4 py-2.5 rounded-md shadow-lg text-sm max-w-sm",
            "animate-in slide-in-from-bottom-2 fade-in-0",
            t.variant === "success"
              ? "bg-primary text-primary-foreground"
              : "bg-destructive text-white"
          )}
        >
          <span className="flex-1">{t.message}</span>
          <button
            type="button"
            onClick={() => setToasts((prev) => prev.filter((x) => x.id !== t.id))}
            className="opacity-70 hover:opacity-100"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
}
