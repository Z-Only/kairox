export type TraceEvent = {
  event_type: string;
};

export function traceLabels(events: TraceEvent[]): string[] {
  return events.map((event) => event.event_type);
}
