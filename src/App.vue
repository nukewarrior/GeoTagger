<script setup lang="ts">
import AppToolbar from "@/components/AppToolbar.vue";
import IconGlyph from "@/components/IconGlyph.vue";
import StatusBar from "@/components/StatusBar.vue";
import { useWorkspaceStore } from "@/stores/workspace";
import ImportWizard from "@/views/ImportWizard.vue";
import SettingsView from "@/views/SettingsView.vue";
import TasksView from "@/views/TasksView.vue";
import WelcomeView from "@/views/WelcomeView.vue";
import WorkspaceView from "@/views/WorkspaceView.vue";
import WriteConfirmView from "@/views/WriteConfirmView.vue";

const store = useWorkspaceStore();
</script>

<template>
  <div class="app-shell">
    <AppToolbar />

    <div class="app-content">
      <Transition name="view-shift" mode="out-in">
        <WelcomeView v-if="store.activeView === 'welcome'" key="welcome" />
        <ImportWizard v-else-if="store.activeView === 'import'" key="import" />
        <WorkspaceView
          v-else-if="store.activeView === 'workspace'"
          key="workspace"
        />
        <WriteConfirmView v-else-if="store.activeView === 'write'" key="write" />
        <TasksView v-else-if="store.activeView === 'tasks'" key="tasks" />
        <SettingsView v-else key="settings" />
      </Transition>
    </div>

    <StatusBar />

    <Transition name="toast">
      <button
        v-if="store.notice"
        class="app-toast notice-toast"
        type="button"
        @click="store.dismissNotice"
      >
        <IconGlyph name="check" />
        <span>{{ store.notice }}</span>
        <IconGlyph name="close" :size="15" />
      </button>
    </Transition>

    <Transition name="toast">
      <button
        v-if="store.error"
        class="app-toast error-toast"
        type="button"
        @click="store.dismissError"
      >
        <IconGlyph name="warning" />
        <span>
          <strong>{{ store.error.message }}</strong>
          <small>{{ store.error.suggestion }}</small>
        </span>
        <IconGlyph name="close" :size="15" />
      </button>
    </Transition>

    <Transition name="fade">
      <div v-if="store.loading" class="loading-scrim" aria-live="polite">
        <span class="loading-orbit"><i /><i /><i /></span>
        <strong>正在安全处理本地数据…</strong>
      </div>
    </Transition>
  </div>
</template>
