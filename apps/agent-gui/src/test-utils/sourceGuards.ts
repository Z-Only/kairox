import { expect } from "vitest";

export type SourceMigrationGuard = {
  required?: string[];
  forbidden?: string[];
  requiredPatterns?: RegExp[];
  forbiddenPatterns?: RegExp[];
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

export function expectSourceToMatch(source: string, patterns: RegExp[]): void {
  for (const pattern of patterns) {
    expect(source, `source should match ${pattern}`).toMatch(pattern);
  }
}

export function expectSourceNotToMatch(source: string, patterns: RegExp[]): void {
  for (const pattern of patterns) {
    expect(source, `source should not match ${pattern}`).not.toMatch(pattern);
  }
}

export function expectSourceMigration(source: string, guard: SourceMigrationGuard): void {
  expectSourceToContain(source, guard.required ?? []);
  expectSourceNotToContain(source, guard.forbidden ?? []);
  expectSourceToMatch(source, guard.requiredPatterns ?? []);
  expectSourceNotToMatch(source, guard.forbiddenPatterns ?? []);
}
