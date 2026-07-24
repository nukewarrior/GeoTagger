<script setup lang="ts">
import { computed } from "vue";

import IconGlyph, { type IconName } from "@/components/IconGlyph.vue";
import { useWorkspaceStore } from "@/stores/workspace";
import type { AppView } from "@/types/domain";

interface Tool {
  label: string;
  icon: IconName;
  view?: AppView;
  accent?: boolean;
  disabledWithoutProject?: boolean;
}

const store = useWorkspaceStore();

const tools: Tool[] = [
  { label: "项目", icon: "project", view: "welcome" },
  {
    label: "导入轨迹",
    icon: "route",
    view: "import",
    disabledWithoutProject: true,
  },
  {
    label: "导入照片",
    icon: "photo",
    view: "import",
    disabledWithoutProject: true,
  },
  {
    label: "时间校准",
    icon: "clock",
    view: "workspace",
    disabledWithoutProject: true,
  },
  {
    label: "坐标转换",
    icon: "transform",
    view: "workspace",
    disabledWithoutProject: true,
  },
  {
    label: "预览匹配",
    icon: "preview",
    view: "workspace",
    accent: true,
    disabledWithoutProject: true,
  },
  {
    label: "写入 EXIF",
    icon: "write",
    view: "write",
    disabledWithoutProject: true,
  },
  {
    label: "导出",
    icon: "export",
    view: "tasks",
    disabledWithoutProject: true,
  },
];

const projectName = computed(
  () => store.project?.project.name ?? "照片地理标记工作台",
);

function activate(tool: Tool): void {
  if (tool.view === "write") {
    void store.prepareWritePlan();
    return;
  }
  if (tool.view) store.setView(tool.view);
}
</script>

<template>
  <header class="app-toolbar">
    <div class="window-brand" aria-label="应用标题">
      <span class="brand-mark">
        <span />
        <span />
        <span />
      </span>
      <strong>{{ projectName }}</strong>
      <span v-if="store.project?.dirty" class="dirty-dot" title="有未保存更改" />
    </div>

    <nav class="tool-strip" aria-label="主要工具">
      <button
        v-for="tool in tools"
        :key="tool.label"
        class="tool-button"
        :class="{
          active: tool.view === store.activeView,
          accent: tool.accent && store.activeView === tool.view,
        }"
        :disabled="tool.disabledWithoutProject && !store.project"
        type="button"
        @click="activate(tool)"
      >
        <IconGlyph :name="tool.icon" :size="17" />
        <span>{{ tool.label }}</span>
      </button>
    </nav>

    <div class="toolbar-actions">
      <button
        class="icon-button"
        type="button"
        title="保存项目"
        :disabled="!store.project"
        @click="store.persistProject"
      >
        <IconGlyph name="save" />
      </button>
      <button
        class="icon-button"
        :class="{ active: store.activeView === 'tasks' }"
        type="button"
        title="任务中心"
        @click="store.setView('tasks')"
      >
        <IconGlyph name="tasks" />
      </button>
      <button
        class="icon-button"
        :class="{ active: store.activeView === 'settings' }"
        type="button"
        title="设置"
        @click="store.setView('settings')"
      >
        <IconGlyph name="settings" />
      </button>
      <button class="icon-button" type="button" title="帮助">
        <IconGlyph name="help" />
      </button>
    </div>
  </header>
</template>
