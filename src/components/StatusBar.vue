<script setup lang="ts">
import { computed } from "vue";

import { useWorkspaceStore } from "@/stores/workspace";

const store = useWorkspaceStore();

const track = computed(() => store.project?.tracks[0]);

function shortUtc(value?: string): string {
  if (!value) return "--";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
    timeZone: "UTC",
  }).format(new Date(value));
}
</script>

<template>
  <footer class="status-bar">
    <template v-if="store.project">
      <span class="status-segment">
        轨迹时间
        <strong>{{ shortUtc(track?.startUtc) }} - {{ shortUtc(track?.endUtc) }}</strong>
      </span>
      <span class="status-segment">
        照片
        <strong>{{ store.statistics.total }}</strong>
      </span>
      <span class="status-segment status-good">
        高置信
        <strong>{{ store.statistics.high }}</strong>
      </span>
      <span class="status-segment status-caution">
        待检查
        <strong>{{ store.statistics.medium + store.statistics.low }}</strong>
      </span>
      <span class="status-segment status-bad">
        未匹配
        <strong>{{ store.statistics.failed }}</strong>
      </span>
      <span class="status-fill" />
      <span class="status-segment">
        {{ store.mapBaseAvailable ? "底图在线" : "离线图层模式" }}
      </span>
      <span class="status-segment">
        {{ store.project.dirty ? "项目有更改" : "项目已保存" }}
      </span>
    </template>
    <template v-else>
      <span>本地处理 · 不上传照片、轨迹或坐标</span>
      <span class="status-fill" />
      <span>GeoTagger 0.1.0 MVP</span>
    </template>
  </footer>
</template>
