<script setup lang="ts">
import { useMcpStore } from "@/stores/mcp";

const { t } = useI18n();
const mcp = useMcpStore();

const props = defineProps<{
  open: boolean;
  mode: "git" | "manual";
}>();

const emit = defineEmits<{
  close: [];
}>();

const serverName = ref("");
const serverDescription = ref("");
const transport = ref<"stdio" | "sse" | "streamable_http">("stdio");
const stdioCommand = ref("");
const stdioArgs = ref("");
const sseUrl = ref("");

function resetForm(): void {
  serverName.value = "";
  serverDescription.value = "";
  transport.value = "stdio";
  stdioCommand.value = "";
  stdioArgs.value = "";
  sseUrl.value = "";
}

function parseArgs(argsText: string): string[] {
  return argsText
    .split(/\s+/)
    .map((arg) => arg.trim())
    .filter(Boolean);
}

async function saveServer(): Promise<void> {
  const trimmedName = serverName.value.trim();
  if (!trimmedName) return;

  const savedServer = await mcp.saveServerSettings({
    name: trimmedName,
    transport:
      transport.value === "stdio"
        ? {
            transport: "stdio",
            command: stdioCommand.value.trim(),
            args: parseArgs(stdioArgs.value),
            env: {}
          }
        : {
            transport: transport.value,
            url: sseUrl.value.trim(),
            headers: {}
          },
    enabled: true,
    description: serverDescription.value.trim() || null
  });

  if (savedServer) {
    emit("close");
  }
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) resetForm();
  }
);
</script>

<template>
  <ModalDialog
    :open="open"
    :title="mode === 'git' ? t('mcp.dialogGitTitle') : t('mcp.dialogManualTitle')"
    :description="mode === 'git' ? t('mcp.dialogGitDesc') : t('mcp.dialogManualDesc')"
    data-test="mcp-add-server-dialog"
    @close="emit('close')"
  >
    <form class="form" data-test="mcp-save" @submit.prevent="saveServer">
      <KxFormField :label="t('mcp.serverName')">
        <KxInput id="mcp-server-name" v-model="serverName" data-test="mcp-form-name" required />
      </KxFormField>

      <template v-if="mode === 'git'">
        <KxFormField :label="t('mcp.gitUrl')">
          <KxInput
            id="mcp-server-git-url"
            v-model="stdioCommand"
            data-test="mcp-form-git-url"
            placeholder="https://github.com/..."
          />
        </KxFormField>
      </template>

      <template v-if="mode === 'manual'">
        <KxFormField :label="t('mcp.description')">
          <KxInput
            id="mcp-server-description"
            v-model="serverDescription"
            data-test="mcp-form-description"
          />
        </KxFormField>

        <fieldset class="form-fieldset">
          <legend>{{ t("mcp.transport") }}</legend>
          <label>
            <input v-model="transport" type="radio" value="stdio" data-test="mcp-form-stdio" />
            stdio
          </label>
          <label>
            <input v-model="transport" type="radio" value="sse" data-test="mcp-form-sse" />
            SSE
          </label>
          <label>
            <input
              v-model="transport"
              type="radio"
              value="streamable_http"
              data-test="mcp-form-streamable-http"
            />
            {{ t("mcp.streamableHttp") }}
          </label>
        </fieldset>

        <template v-if="transport === 'stdio'">
          <KxFormField :label="t('mcp.command')">
            <KxInput id="mcp-server-command" v-model="stdioCommand" data-test="mcp-form-command" />
          </KxFormField>
          <KxFormField :label="t('mcp.arguments')">
            <KxInput id="mcp-server-args" v-model="stdioArgs" data-test="mcp-form-args" />
          </KxFormField>
        </template>
        <template v-else>
          <KxFormField :label="t('mcp.url')">
            <KxInput id="mcp-server-url" v-model="sseUrl" type="url" data-test="mcp-form-url" />
          </KxFormField>
        </template>
      </template>
    </form>

    <template #footer>
      <KxButton @click="emit('close')">
        {{ t("common.cancel") }}
      </KxButton>
      <KxButton
        variant="primary"
        type="submit"
        :disabled="mcp.settingsLoading || !serverName.trim()"
        data-test="mcp-save-button"
        @click="saveServer"
      >
        {{ mcp.settingsLoading ? t("mcp.saving") : t("mcp.saveServer") }}
      </KxButton>
    </template>
  </ModalDialog>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 0;
}

.form-fieldset {
  display: flex;
  gap: 12px;
  padding: 0;
  border: 0;
}
</style>
