<script setup lang="ts">
import { computed } from "vue";
import { useData } from "vitepress";
import release from "../../cache/release.json";

type ReleaseAsset = {
  name: string;
  browser_download_url: string;
  size?: number;
};

type ReleaseInfo = {
  tagName?: string;
  publishedAt?: string;
  assets?: ReleaseAsset[];
};

const { lang } = useData();

const info = release as ReleaseInfo;

const hasRelease = computed(() => Boolean(info && info.tagName));

const isZh = computed(() => lang.value.startsWith("zh"));

const labels = computed(() => {
  if (isZh.value) {
    return {
      title: "最新发布",
      published: "发布时间",
      noRelease: "查看 GitHub 上的最新发布 →",
      downloads: "下载"
    };
  }
  return {
    title: "Latest release",
    published: "Published",
    noRelease: "Latest release →",
    downloads: "Downloads"
  };
});

const platformAssets = computed(() => {
  if (!hasRelease.value || !info.assets) {
    return [] as Array<{ label: string; url: string }>;
  }
  const wanted: Array<{ label: string; re: RegExp }> = [
    { label: "macOS (Apple Silicon)", re: /(aarch64|arm64).*(\.dmg|\.app\.tar\.gz)$/i },
    { label: "macOS (Intel)", re: /(x86_64|x64|intel).*(\.dmg|\.app\.tar\.gz)$/i },
    { label: "Linux (AppImage)", re: /\.AppImage$/i },
    { label: "Linux (deb)", re: /\.deb$/i },
    { label: "Linux (rpm)", re: /\.rpm$/i },
    { label: "Windows (MSI)", re: /\.msi$/i },
    { label: "Windows (EXE)", re: /\.exe$/i }
  ];
  const matched: Array<{ label: string; url: string }> = [];
  for (const w of wanted) {
    const asset = info.assets.find((a) => w.re.test(a.name));
    if (asset) {
      matched.push({ label: w.label, url: asset.browser_download_url });
    }
  }
  return matched;
});

const releaseUrl = "https://github.com/Z-Only/kairox/releases/latest";

const publishedDate = computed(() => {
  if (!info.publishedAt) return "";
  try {
    return new Date(info.publishedAt).toISOString().slice(0, 10);
  } catch {
    return info.publishedAt;
  }
});
</script>

<template>
  <section v-if="hasRelease" class="kairox-release-banner">
    <div class="kairox-release-banner__header">
      <strong class="kairox-release-banner__title">{{ labels.title }}: {{ info.tagName }}</strong>
      <span v-if="publishedDate" class="kairox-release-banner__date"
        >{{ labels.published }}: {{ publishedDate }}</span
      >
      <a
        class="kairox-release-banner__more"
        :href="releaseUrl"
        target="_blank"
        rel="noopener noreferrer"
        >{{ labels.noRelease }}</a
      >
    </div>
    <ul v-if="platformAssets.length" class="kairox-release-banner__assets">
      <li v-for="asset in platformAssets" :key="asset.url">
        <a :href="asset.url" target="_blank" rel="noopener noreferrer">{{ asset.label }}</a>
      </li>
    </ul>
  </section>
  <section v-else class="kairox-release-banner kairox-release-banner--fallback">
    <a :href="releaseUrl" target="_blank" rel="noopener noreferrer">{{ labels.noRelease }}</a>
  </section>
</template>
