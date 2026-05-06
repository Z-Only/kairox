import {
  mount as baseMount,
  type ComponentMountingOptions
} from "@vue/test-utils";
import type { Component } from "vue";
import { createPinia, setActivePinia } from "pinia";
import { createI18n } from "vue-i18n";
import { createRouter, createMemoryHistory } from "vue-router";
import en from "@/locales/en.json";
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
 * `routes` array, which includes lazy view imports) plus an English-only
 * `createI18n` instance. For tests that only exercise a store or a leaf
 * component this is unnecessary overhead and noise in the failure surface
 * (router/i18n config bugs masquerading as component bugs).
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
export function mountWithPlugins<T extends Component>(
  comp: T,
  options: ComponentMountingOptions<T> = {}
) {
  const pinia = createPinia();
  setActivePinia(pinia);
  const i18n = createI18n({
    legacy: false,
    locale: "en",
    messages: { en }
  });
  const router = createRouter({ history: createMemoryHistory(), routes });
  return baseMount(comp, {
    ...options,
    global: {
      plugins: [pinia, i18n, router],
      ...(options.global ?? {})
    }
  });
}
