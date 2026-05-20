import { expect } from "vitest";

export type SourceMigrationGuard = {
  required?: string[];
  forbidden?: string[];
};

export function expectSourceToContain(source: string, fragments: string[]): void {
  for (const fragment of fragments) {
    expect(source, `source should contain ${fragment}`).toContain(fragment);
  }
}

export function expectSourceNotToContain(source: string, fragments: string[]): void {
  for (const fragment of fragments) {
    expect(source, `source should not contain ${fragment}`).not.toContain(fragment);
  }
}

export function expectSourceMigration(source: string, guard: SourceMigrationGuard): void {
  expectSourceToContain(source, guard.required ?? []);
  expectSourceNotToContain(source, guard.forbidden ?? []);
}
