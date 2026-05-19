// Thin compatibility shim over the Pinia trace store.
// All state and logic lives in `@/stores/trace`; this module re-exports
// the same `traceState`, `applyTraceEvent`, and `clearTrace` API so
// existing consumers need no changes.
import { useTraceStore as usePiniaTraceStore } from "@/stores/trace";
import type { DomainEvent } from "@/types";

function getStore() {
  return usePiniaTraceStore();
}

/**
 * Reactive trace state. Backed by the Pinia `"trace"` store, exposed as a
 * Proxy so that direct property access (`traceState.entries`,
 * `traceState.density`) delegates to the store instance without requiring
 * callers to invoke `useStore()`.
 */
export const traceState = new Proxy({} as ReturnType<typeof usePiniaTraceStore>, {
  get(_target, prop, receiver) {
    return Reflect.get(getStore(), prop, receiver);
  },
  set(_target, prop, value, receiver) {
    return Reflect.set(getStore(), prop, value, receiver);
  },
  has(_target, prop) {
    return prop in getStore();
  },
  ownKeys(_target) {
    return Reflect.ownKeys(getStore());
  },
  getOwnPropertyDescriptor(_target, prop) {
    return Reflect.getOwnPropertyDescriptor(getStore(), prop);
  }
});

export function applyTraceEvent(event: DomainEvent) {
  getStore().applyTraceEvent(event);
}

export function clearTrace() {
  getStore().clearTrace();
}
