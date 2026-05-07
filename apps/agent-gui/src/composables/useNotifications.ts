import { useMessage } from "naive-ui";
import { useUiStore, type NotificationLevel } from "@/stores/ui";

/**
 * Bridges the ui store (source of truth for in-app notifications) with
 * NaiveUI's transient `useMessage()` toasts.
 *
 * **Single visual path.** `notify()` writes to the `ui.notifications` store
 * exactly once. The `<NotificationToast />` adapter mounted under
 * `<NMessageProvider>` watches that same store and forwards each new entry
 * to `useMessage()` for rendering. Therefore: **do NOT also call
 * `message.*` directly elsewhere** — every transient toast must flow
 * through `useNotifications.notify()` so the store stays the single source
 * of truth and no event renders twice.
 *
 * **Where to call.** Must be called from inside a component that lives
 * below `<NMessageProvider>` (i.e. anywhere under `AppLayout.vue`). When
 * called from outside that subtree (a store, a router guard, or a
 * top-level service) `useMessage()` would normally throw and crash the
 * host component; the try/catch below downgrades that to a one-line
 * `console.error` and returns a `notify()` that only writes to the store
 * (the `<NotificationToast />` adapter under the provider will still pick
 * the entry up if mounted later).
 */
export function useNotifications() {
  const ui = useUiStore();

  // `useMessage()` throws when called outside an `<NMessageProvider>`
  // subtree. We swallow that synchronously so misuse from a store or a
  // top-level module degrades gracefully instead of crashing the caller.
  // The return value is intentionally unused here — the adapter under
  // `<NMessageProvider>` is what actually paints the toast. Calling
  // `useMessage()` here only validates that the provider is present; the
  // failure mode is reported once at composable construction time rather
  // than at first `notify()` call.
  //
  // We additionally remember the provider state so subsequent `notify()`
  // calls can keep a console trace alive for `error`-level events even
  // after the visual layer has fallen back to store-only mode (otherwise
  // the construction-time `console.error` is the only diagnostic for the
  // entire lifetime of the host component).
  let providerAvailable = true;
  try {
    useMessage();
  } catch {
    providerAvailable = false;
    console.error(
      "[useNotifications] useMessage() is unavailable — useNotifications must be called inside the AppLayout subtree (below <NMessageProvider>). Falling back to store-only notifications."
    );
  }

  function notify(level: NotificationLevel, content: string) {
    // Persistent log first; this is the source of truth and the visual
    // layer (the `<NotificationToast />` adapter) consumes the same store.
    ui.pushNotification(level, content);
    // In degraded mode the adapter never paints — leave a per-event
    // console trace for `error`-level events so failures still surface to
    // developers tailing the dev console / Tauri logs.
    if (!providerAvailable && level === "error") {
      console.error("[useNotifications] (degraded) error:", content);
    }
  }

  return { notify };
}
