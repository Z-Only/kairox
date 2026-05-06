import {
  mount as baseMount,
  type ComponentMountingOptions
} from "@vue/test-utils";
import { defineComponent, h, type Component } from "vue";
import { createPinia, setActivePinia } from "pinia";
import { createI18n } from "vue-i18n";
import { createRouter, createMemoryHistory, type Router } from "vue-router";
import {
  NConfigProvider,
  NMessageProvider,
  NDialogProvider,
  NNotificationProvider,
  NLoadingBarProvider
} from "naive-ui";
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
 * - NaiveUI service hooks (`useMessage`, `useDialog`, `useNotification`,
 *   `useLoadingBar`) — pass `withNaiveProviders: true` to wrap the mount in
 *   the same provider stack `AppLayout.vue` mounts at runtime.
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
 * imports `useI18n`, `useRoute`/`useRouter`, NaiveUI service hooks, or
 * renders a `<RouterLink>`.
 */
export interface MountWithPluginsOptions<T> {
  /** Extra `@vue/test-utils` mount options merged into `global`. */
  mount?: ComponentMountingOptions<T>;
  /**
   * Wrap the component under test in NaiveUI's provider stack
   * (`NConfigProvider` → `NLoadingBarProvider` → `NMessageProvider` →
   * `NDialogProvider` → `NNotificationProvider`). Required for any
   * component (or composable) that calls `useMessage()`, `useDialog()`,
   * `useNotification()`, or `useLoadingBar()`. Default `false`.
   */
  withNaiveProviders?: boolean;
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
  // (`{ mount?, withNaiveProviders?, initialRoute? }`) or the legacy
  // `ComponentMountingOptions<T>` shape. The new shape is identified by the
  // presence of any of its three known keys.
  const isExtendedOptions =
    "mount" in options ||
    "withNaiveProviders" in options ||
    "initialRoute" in options;
  const extended = (
    isExtendedOptions ? options : {}
  ) as MountWithPluginsOptions<T>;
  const mountOpts: ComponentMountingOptions<T> = isExtendedOptions
    ? (extended.mount ?? {})
    : (options as ComponentMountingOptions<T>);

  const pinia = createPinia();
  setActivePinia(pinia);
  const i18n = createI18n({
    legacy: false,
    locale: "en",
    fallbackLocale: "en",
    messages: { en, "zh-CN": zhCN }
  });
  const router = createRouter({ history: createMemoryHistory(), routes });

  const target = extended.withNaiveProviders
    ? defineComponent({
        name: "NaiveProviderHarness",
        components: { Inner: comp },
        setup() {
          return () =>
            h(NConfigProvider, null, {
              default: () =>
                h(NLoadingBarProvider, null, {
                  default: () =>
                    h(NMessageProvider, null, {
                      default: () =>
                        h(NDialogProvider, null, {
                          default: () =>
                            h(NNotificationProvider, null, {
                              default: () => h(comp as Component)
                            })
                        })
                    })
                })
            });
        }
      })
    : comp;

  const wrapper = baseMount(target as T, {
    ...mountOpts,
    global: {
      plugins: [pinia, i18n, router],
      ...(mountOpts.global ?? {})
    }
  });

  if (isExtendedOptions) {
    return { wrapper, router } as MountWithPluginsResult<T>;
  }
  return wrapper;
}
