import { h, type App } from "vue";
import type { Theme } from "vitepress";
import DefaultTheme from "vitepress/theme";
import FeedbackBlock from "./components/FeedbackBlock.vue";
import LocaleRedirect from "./components/LocaleRedirect.vue";
import MediaLightbox from "./components/MediaLightbox.vue";
import ReleaseBanner from "./components/ReleaseBanner.vue";
import ThemeScreenshot from "./components/ThemeScreenshot.vue";
import "./custom.css";

const theme: Theme = {
  extends: DefaultTheme,
  Layout: () => {
    return [
      h(DefaultTheme.Layout, null, {
        "doc-after": () => h(FeedbackBlock)
      }),
      h(LocaleRedirect),
      h(MediaLightbox)
    ];
  },
  enhanceApp({ app }: { app: App }) {
    app.component("FeedbackBlock", FeedbackBlock);
    app.component("LocaleRedirect", LocaleRedirect);
    app.component("MediaLightbox", MediaLightbox);
    app.component("ReleaseBanner", ReleaseBanner);
    app.component("ThemeScreenshot", ThemeScreenshot);
  }
};

export default theme;
