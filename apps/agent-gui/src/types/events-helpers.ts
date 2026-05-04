/**
 * Type-safe event handling utilities for EventPayload.
 *
 * These helpers leverage the auto-generated EventPayload discriminated union
 * to provide exhaustive pattern matching and type narrowing.
 */

import type { EventPayload } from "../generated/events";

/** Extract a specific EventPayload variant by its type tag. */
export type ExtractPayload<T extends EventPayload["type"]> = Extract<
  EventPayload,
  { type: T }
>;

/**
 * Exhaustive event handler map.
 * TypeScript will error if a new EventPayload variant is added but not handled.
 * Each handler receives the narrowed payload type for its variant.
 */
export type EventPayloadHandlers<R = void> = {
  [K in EventPayload["type"]]: (payload: ExtractPayload<K>) => R;
};

/**
 * Partial event handler map.
 * Only handle the events you care about. Unhandled variants are ignored.
 */
export type PartialEventPayloadHandlers<R = void> = {
  [K in EventPayload["type"]]?: (payload: ExtractPayload<K>) => R;
};

/**
 * Process an EventPayload with exhaustive pattern matching.
 * If a new variant is added to EventPayload, TypeScript will error
 * until a handler is added for it.
 */
export function matchPayload<R>(
  payload: EventPayload,
  handlers: EventPayloadHandlers<R>
): R {
  const handler = handlers[payload.type] as (p: EventPayload) => R;
  return handler(payload);
}

/**
 * Process an EventPayload with partial pattern matching.
 * Unhandled variants are silently ignored.
 */
export function matchPartialPayload<R>(
  payload: EventPayload,
  handlers: PartialEventPayloadHandlers<R>
): R | undefined {
  const handler = handlers[payload.type] as
    | ((p: EventPayload) => R)
    | undefined;
  return handler?.(payload);
}
