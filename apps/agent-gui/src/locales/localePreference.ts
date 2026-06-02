export type SupportedLocale = "system" | "en" | "zh-CN";
export type ResolvedLocale = Exclude<SupportedLocale, "system">;

export const LOCALE_STORAGE_KEY = "kairox.locale";
const LOCALE_EXPLICIT_STORAGE_KEY = "kairox.locale.explicit";

function browserStorage(): Storage | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function normalizeStoredLocale(value: unknown): SupportedLocale {
  return value === "system" || value === "zh-CN" || value === "en" ? value : "system";
}

export function migrateLegacyLocaleDefault(storage: Storage | null = browserStorage()): void {
  if (!storage) return;
  try {
    const stored = storage.getItem(LOCALE_STORAGE_KEY);
    const explicit = storage.getItem(LOCALE_EXPLICIT_STORAGE_KEY);
    if ((stored === "en" || stored === "zh-CN") && explicit !== "true") {
      storage.setItem(LOCALE_STORAGE_KEY, "system");
    }
  } catch {
    // Storage access can fail under restricted browser settings; keep the
    // runtime default rather than blocking app startup.
  }
}

export function readStoredLocalePreference(): SupportedLocale {
  const storage = browserStorage();
  if (!storage) return "system";
  migrateLegacyLocaleDefault(storage);
  try {
    return normalizeStoredLocale(storage.getItem(LOCALE_STORAGE_KEY));
  } catch {
    return "system";
  }
}

export function markLocalePreferenceExplicit(): void {
  const storage = browserStorage();
  if (!storage) return;
  try {
    storage.setItem(LOCALE_EXPLICIT_STORAGE_KEY, "true");
  } catch {
    // Best-effort marker only.
  }
}

export function resolveLocalePreference(
  preference: SupportedLocale,
  browserLanguage = typeof navigator === "undefined" ? "" : navigator.language
): ResolvedLocale {
  if (preference !== "system") return preference;
  return browserLanguage.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}
