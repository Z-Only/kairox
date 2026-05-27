import { h, type App } from "vue";
import type { Theme } from "vitepress";
import DefaultTheme from "vitepress/theme";
import FeedbackBlock from "./components/FeedbackBlock.vue";
import ReleaseBanner from "./components/ReleaseBanner.vue";
import "./custom.css";

const theme: Theme = {
  extends: DefaultTheme,
  Layout: () => {
    return h(DefaultTheme.Layout, null, {
      "doc-after": () => h(FeedbackBlock)
    });
  },
  enhanceApp({ app }: { app: App }) {
    app.component("FeedbackBlock", FeedbackBlock);
    app.component("ReleaseBanner", ReleaseBanner);
  }
};

export default theme;
