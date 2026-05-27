<script setup lang="ts">
import { computed } from "vue";
import { useData, useRoute } from "vitepress";

const { lang } = useData();
const route = useRoute();

const repoUrl = "https://github.com/Z-Only/kairox";
const category = "site-feedback";

const labels = computed(() => {
  if (lang.value.startsWith("zh")) {
    return {
      prompt: "本页对你有帮助吗?",
      helpful: "有帮助",
      needsImprovement: "需要改进",
      hint: "会跳转到 GitHub Discussions 创建一条预填的反馈。"
    };
  }
  return {
    prompt: "Was this page helpful?",
    helpful: "Helpful",
    needsImprovement: "Needs improvement",
    hint: "Opens a prefilled GitHub Discussion in a new tab."
  };
});

function feedbackUrl(kind: "helpful" | "needs-improvement"): string {
  const title = encodeURIComponent(`[site feedback] ${kind}: ${route.path}`);
  const body = encodeURIComponent(
    [
      `Page: ${route.path}`,
      `Locale: ${lang.value}`,
      "",
      kind === "helpful"
        ? "What worked well on this page:"
        : "What was missing, unclear, or wrong on this page:",
      "",
      ""
    ].join("\n")
  );
  return `${repoUrl}/discussions/new?category=${category}&title=${title}&body=${body}`;
}
</script>

<template>
  <aside class="kairox-feedback" aria-label="Page feedback">
    <p class="kairox-feedback__prompt">{{ labels.prompt }}</p>
    <div class="kairox-feedback__buttons">
      <a
        class="kairox-feedback__button kairox-feedback__button--ok"
        :href="feedbackUrl('helpful')"
        target="_blank"
        rel="noopener noreferrer"
      >
        {{ labels.helpful }}
      </a>
      <a
        class="kairox-feedback__button kairox-feedback__button--bad"
        :href="feedbackUrl('needs-improvement')"
        target="_blank"
        rel="noopener noreferrer"
      >
        {{ labels.needsImprovement }}
      </a>
    </div>
    <p class="kairox-feedback__hint">{{ labels.hint }}</p>
  </aside>
</template>
