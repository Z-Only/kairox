import { useUiStore } from "@/stores/ui";

export function useToast() {
  const ui = useUiStore();

  return {
    success: (message: string, duration?: number) => ui.addToast(message, "success", duration),
    error: (message: string, duration?: number) => ui.addToast(message, "error", duration ?? 8000),
    info: (message: string, duration?: number) => ui.addToast(message, "info", duration),
    warning: (message: string, duration?: number) => ui.addToast(message, "warning", duration)
  };
}
