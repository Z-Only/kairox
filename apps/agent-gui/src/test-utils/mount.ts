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
