<script setup lang="ts">
import { ref, computed, watch } from "vue";
import {
  NDrawer,
  NDrawerContent,
  NCard,
  NDescriptions,
  NDescriptionsItem,
  NInput,
  NCheckbox,
  NButton,
  NText,
  NSpace
} from "naive-ui";
import type {
  ServerEntryResponse,
  InstallRequestPayload
} from "../../generated/commands";
import { useCatalogStore } from "@/stores/catalog";
import {
  parseRequirements,
  parseDefaultEnv
} from "../../composables/useMarketplace";
import RuntimeMissingHint from "./RuntimeMissingHint.vue";

const catalog = useCatalogStore();
const props = defineProps<{ entry: ServerEntryResponse }>();
const emit = defineEmits<{ close: [] }>();

const requirements = computed(() => parseRequirements(props.entry));
const envSpec = computed(() => parseDefaultEnv(props.entry));
const overrides = ref<Record<string, string>>({});
// Trust grant must be opt-in: catalog "verified" means the *distribution
// channel* is trusted, not that runtime tool calls should bypass the
// PermissionCenter. Default to false and let the user opt in explicitly.
const trustGrant = ref(false);
const autoStart = ref(true);

// Re-initialise local form state whenever the selected entry changes.
// Using `watch(..., immediate: true)` instead of `onMounted` so that switching
// entries without unmounting the drawer (e.g. future keep-alive use) still
// resets overrides + checkboxes cleanly.
watch(
  () => props.entry.id,
  () => {
    const next: Record<string, string> = {};
    for (const spec of envSpec.value) {
      next[spec.key] = spec.default ?? "";
    }
    overrides.value = next;
    trustGrant.value = false;
    autoStart.value = true;
  },
  { immediate: true }
);

async function onInstall() {
  const req: InstallRequestPayload = {
    catalog_id: props.entry.id,
    source: props.entry.source,
    server_id_override: null,
    env_overrides: overrides.value,
    trust_grant: trustGrant.value,
    auto_start: autoStart.value
  };
  // InstallProgress lives on MarketplaceView so closing this drawer (which
  // unmounts CatalogDetail) does not unmount the in-flight progress modal.
  catalog.requestInstallProgress(props.entry.id);
  await catalog.installEntry(req);
}

// NDrawer's `show` is two-way bound; flipping it to false fires
// `update:show`, which we forward as the legacy `close` event so the
// parent (`CatalogList`) can drop its `selected` ref without changing
// its API.
function onShowUpdate(next: boolean) {
  if (!next) emit("close");
}
</script>

<template>
  <!-- NDrawer placement="right" mirrors the previous `position: fixed; right: 0`
       slide-out detail panel. data-test="catalog-detail" stays attached to
       the inner container so the existing e2e selector hits a real element
       even though NDrawer renders in a teleport. -->
  <NDrawer
    :show="true"
    :width="480"
    placement="right"
    :auto-focus="false"
    :mask-closable="true"
    @update:show="onShowUpdate"
  >
    <NDrawerContent
      :title="entry.display_name"
      closable
      :native-scrollbar="false"
    >
      <div data-test="catalog-detail" class="catalog-detail">
        <NText depth="2">{{ entry.description }}</NText>
        <a
          v-if="entry.homepage"
          :href="entry.homepage"
          target="_blank"
          rel="noopener"
          class="homepage-link"
        >
          Homepage
        </a>

        <NCard size="small" title="Requirements" :bordered="true">
          <RuntimeMissingHint :requirements="requirements" />
        </NCard>

        <NCard size="small" title="Configure" :bordered="true">
          <NDescriptions
            v-if="envSpec.length > 0"
            label-placement="top"
            :column="1"
            size="small"
            bordered
          >
            <NDescriptionsItem v-for="spec in envSpec" :key="spec.key">
              <template #label>
                {{ spec.label }}<span v-if="spec.required">*</span>
              </template>
              <NInput
                v-model:value="overrides[spec.key]"
                :type="spec.secret ? 'password' : 'text'"
                :placeholder="spec.default ?? ''"
                :show-password-on="spec.secret ? 'click' : undefined"
                size="small"
                :input-props="{ 'data-test': `env-${spec.key}` }"
              />
              <NText v-if="spec.description" depth="3" class="env-help">
                {{ spec.description }}
              </NText>
            </NDescriptionsItem>
          </NDescriptions>
          <NText v-else depth="3">No configurable environment variables.</NText>
        </NCard>

        <NCard size="small" title="Options" :bordered="true">
          <NSpace vertical :size="6">
            <NCheckbox v-model:checked="trustGrant">
              Trust this server (skip per-tool permission prompts)
            </NCheckbox>
            <NText
              v-if="entry.trust === 'verified'"
              depth="3"
              class="hint-verified"
            >
              This entry comes from a verified source. You can grant runtime
              trust to skip permission prompts, but it remains opt-in.
            </NText>
            <NCheckbox v-model:checked="autoStart">
              Start after install
            </NCheckbox>
          </NSpace>
        </NCard>
      </div>

      <template #footer>
        <NSpace>
          <NButton
            type="primary"
            size="small"
            data-test="catalog-install"
            @click="onInstall"
          >
            Install
          </NButton>
          <NButton size="small" @click="emit('close')">Close</NButton>
        </NSpace>
      </template>
    </NDrawerContent>
  </NDrawer>
</template>

<style scoped>
.catalog-detail {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.homepage-link {
  font-size: 13px;
}
.env-help {
  display: block;
  margin-top: 4px;
  font-size: 11px;
}
.hint-verified {
  font-size: 12px;
}
</style>
