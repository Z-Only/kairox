<script setup lang="ts">
defineProps<{
  title: string;
  message: string;
  confirmLabel?: string;
  confirmDanger?: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();
</script>

<template>
  <div class="dialog-backdrop" @click.self="emit('cancel')">
    <div class="dialog-box">
      <h3>{{ title }}</h3>
      <p>{{ message }}</p>
      <div class="dialog-actions">
        <button class="btn-cancel" @click="emit('cancel')">Cancel</button>
        <button
          :class="['btn-confirm', { 'btn-danger': confirmDanger }]"
          @click="emit('confirm')"
        >
          {{ confirmLabel || "Confirm" }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dialog-backdrop {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
}
.dialog-box {
  background: white;
  border-radius: 8px;
  padding: 20px 24px;
  min-width: 320px;
  max-width: 420px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2);
}
.dialog-box h3 {
  margin: 0 0 8px;
  font-size: 15px;
}
.dialog-box p {
  margin: 0 0 16px;
  color: #555;
  font-size: 13px;
  line-height: 1.5;
}
.dialog-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.btn-cancel {
  padding: 6px 16px;
  background: #f5f5f5;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn-cancel:hover {
  background: #eee;
}
.btn-confirm {
  padding: 6px 16px;
  background: #0077cc;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn-confirm:hover {
  background: #0066b3;
}
.btn-danger {
  background: #cc3333;
}
.btn-danger:hover {
  background: #b32828;
}
</style>
