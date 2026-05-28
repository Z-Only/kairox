import { h, type App } from "vue";
import type { Theme } from "vitepress";
import DefaultTheme from "vitepress/theme";
import FeedbackBlock from "./components/FeedbackBlock.vue";
import MediaLightbox from "./components/MediaLightbox.vue";
import ReleaseBanner from "./components/ReleaseBanner.vue";
import "./custom.css";

const theme: Theme = {
  extends: DefaultTheme,
  Layout: () => {
    return [
      h(DefaultTheme.Layout, null, {
        "doc-after": () => h(FeedbackBlock)
      }),
      h(MediaLightbox)
    ];
  },
  enhanceApp({ app }: { app: App }) {
    app.component("FeedbackBlock", FeedbackBlock);
    app.component("MediaLightbox", MediaLightbox);
    app.component("ReleaseBanner", ReleaseBanner);
  }
};

export default theme;
