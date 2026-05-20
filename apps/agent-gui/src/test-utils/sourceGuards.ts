import { expect } from "vitest";

export function expectSourceNotToContain(source: string, fragments: string[]): void {
  for (const fragment of fragments) {
    expect(source, `source should not contain ${fragment}`).not.toContain(fragment);
  }
}
