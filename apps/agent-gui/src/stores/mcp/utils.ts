export type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: string };

export function formatError(caughtError: unknown): string {
  return caughtError instanceof Error ? caughtError.message : String(caughtError);
}

export function isCommandResult<T>(result: T | CommandResult<T>): result is CommandResult<T> {
  return (
    typeof result === "object" &&
    result !== null &&
    "status" in result &&
    (result.status === "ok" || result.status === "error")
  );
}

export async function unwrapCommandResult<T>(
  resultPromise: Promise<T | CommandResult<T>>
): Promise<T> {
  const result = await resultPromise;
  if (!isCommandResult(result)) {
    return result;
  }
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}
