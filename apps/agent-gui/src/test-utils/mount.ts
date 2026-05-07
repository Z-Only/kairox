// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). This test-utils module is plain `.ts`, so
// `defineComponent`, `h`, `createI18n`, `createRouter`, etc. must be
// imported explicitly.
import { mount as baseMount, type ComponentMountingOptions } from "@vue/test-utils";
import type { Component } from "vue";
import { createPinia, setActivePinia } from "pinia";
import { createI18n } from "vue-i18n";
import { createRouter, createMemoryHistory, type Router } from "vue-router";
import en from "@/locales/en.json";
import zhCN from "@/locales/zh-CN.json";
import { routes } from "@/router/routes";

/**
 * Mount a component with the full plugin stack (Pinia + vue-i18n + vue-router).
 *
 * **When to use:**
 * Component tests that need any of the following at render time:
 * - `<RouterLink>` / `<RouterView>` resolution (e.g. layout / nav components)
 * - `$t(...)` / `useI18n()` translation lookups
 * - A fresh, isolated Pinia instance bound via `setActivePinia` so each test
 *   starts from a clean store state
 *
 * **What it costs:**
 * Every call constructs a brand-new memory-history router (loading the full
 * `routes` array, which includes lazy view imports) plus an i18n instance.
 * For tests that only exercise a store or a leaf component this is
 * unnecessary overhead and noise in the failure surface (router/i18n config
 * bugs masquerading as component bugs).
 *
 * **Recommended alternative for store-only or pure-leaf tests:**
 *
 * ```ts
 * import { mount } from "@vue/test-utils";
 * import { setActivePinia, createPinia } from "pinia";
 *
 * setActivePinia(createPinia());
 * const wrapper = mount(MyComponent, { props: { ... } });
 * ```
 *
 * Reach for `mountWithPlugins` only when the component under test actually
 * imports `useI18n`, `useRoute`/`useRouter`, or renders a `<RouterLink>`.
 */
export interface MountWithPluginsOptions<T> {
  /** Extra `@vue/test-utils` mount options merged into `global`. */
  mount?: ComponentMountingOptions<T>;
  /**
   * When `true`, do NOT create a new Pinia instance — assume the caller
   * has already called `setActivePinia(createPinia())` in `beforeEach`.
   * Required for specs that follow the "useStore() → set state →
   * mount()" pattern; otherwise `mountWithPlugins`'s internal
   * `createPinia()` resets all stores and the pre-mount state mutations
   * are lost. Default `false`.
   */
  reusePinia?: boolean;
  /** Initial route to push before mount (and `await router.isReady()`). */
  initialRoute?: string;
}

export interface MountWithPluginsResult<T> {
  wrapper: ReturnType<typeof baseMount<T>>;
  router: Router;
}

export function mountWithPlugins<T extends Component>(
  comp: T,
  options: ComponentMountingOptions<T> = {}
): ReturnType<typeof baseMount<T>>;
export function mountWithPlugins<T extends Component>(
  comp: T,
  options: MountWithPluginsOptions<T>
): MountWithPluginsResult<T>;
export function mountWithPlugins<T extends Component>(
  comp: T,
  options: ComponentMountingOptions<T> | MountWithPluginsOptions<T> = {}
) {
  // Detect whether the caller passed the new `MountWithPluginsOptions` shape
  // (`{ mount?, reusePinia?, initialRoute? }`) or the legacy
  // `ComponentMountingOptions<T>` shape. The new shape is identified by the
  // presence of any of its known keys.
  const isExtendedOptions =
    "mount" in options || "reusePinia" in options || "initialRoute" in options;
  const extended = (isExtendedOptions ? options : {}) as MountWithPluginsOptions<T>;
  const mountOpts: ComponentMountingOptions<T> = isExtendedOptions
    ? (extended.mount ?? {})
    : (options as ComponentMountingOptions<T>);

  // When `reusePinia: true`, the caller has already done
  // `setActivePinia(createPinia())` in beforeEach (and likely mutated
  // store state before calling us). Creating a new pinia here would
  // wipe those mutations.
  const pinia = extended.reusePinia ? null : createPinia();
  if (pinia) setActivePinia(pinia);
  const i18n = createI18n({
    legacy: false,
    locale: "en",
    fallbackLocale: "en",
    messages: { en, "zh-CN": zhCN }
  });
  const router = createRouter({ history: createMemoryHistory(), routes });

  // Only register the freshly-created pinia plugin; in `reusePinia`
  // mode the caller's already-active pinia is picked up via
  // `getActivePinia()` inside `setup()` — no plugin needed.
  const plugins = pinia ? [pinia, i18n, router] : [i18n, router];
  const wrapper = baseMount(comp as T, {
    ...mountOpts,
    global: {
      plugins,
      ...mountOpts.global
    }
  });

  if (isExtendedOptions) {
    return { wrapper, router } as MountWithPluginsResult<T>;
  }
  return wrapper;
}
