<script setup lang="ts">
import { computed } from "vue";

import IconGlyph from "@/components/IconGlyph.vue";
import { useWorkspaceStore } from "@/stores/workspace";
import type { MatchStatus, PhotoRecord } from "@/types/domain";

const store = useWorkspaceStore();

const track = computed(() => store.project?.tracks[0]);

const filterOptions: Array<{ value: MatchStatus | "ALL"; label: string }> = [
  { value: "ALL", label: "全部状态" },
  { value: "MATCHED_HIGH", label: "高置信" },
  { value: "MATCHED_MEDIUM", label: "中置信" },
  { value: "MATCHED_LOW", label: "低置信" },
  { value: "OUT_OF_RANGE", label: "超出范围" },
  { value: "SEGMENT_GAP", label: "轨迹中断" },
  { value: "NO_CAPTURE_TIME", label: "缺少时间" },
];

function matchStatus(photoId: string): MatchStatus {
  return store.matchesByPhotoId.get(photoId)?.status ?? "NO_CAPTURE_TIME";
}

function statusLabel(status: MatchStatus): string {
  const labels: Record<MatchStatus, string> = {
    MATCHED_HIGH: "匹配成功",
    MATCHED_MEDIUM: "待确认",
    MATCHED_LOW: "低置信度",
    OUT_OF_RANGE: "超出轨迹范围",
    NO_CAPTURE_TIME: "缺少拍摄时间",
    SEGMENT_GAP: "轨迹中断",
  };
  return labels[status];
}

function statusTone(status: MatchStatus): string {
  if (status === "MATCHED_HIGH") return "good";
  if (status === "MATCHED_MEDIUM") return "medium";
  if (status === "MATCHED_LOW") return "low";
  if (status === "NO_CAPTURE_TIME") return "muted";
  return "bad";
}

function formatCapture(photo: PhotoRecord): string {
  if (!photo.captureLocal) return "无有效拍摄时间";
  return photo.captureLocal.replace("T", " ").slice(0, 19);
}

function formatDuration(start?: string, end?: string): string {
  if (!start || !end) return "--";
  const totalMinutes = Math.round(
    (new Date(end).getTime() - new Date(start).getTime()) / 60_000,
  );
  return `${Math.floor(totalMinutes / 60)}h ${totalMinutes % 60}m`;
}
</script>

<template>
  <aside class="photo-rail">
    <section class="project-summary">
      <div class="section-heading">
        <span>项目</span>
        <button type="button" aria-label="编辑项目">
          <IconGlyph name="settings" :size="15" />
        </button>
      </div>
      <strong>{{ store.project?.project.name }}</strong>
      <p>{{ store.project?.project.directory }}</p>
    </section>

    <section class="track-section">
      <div class="section-heading">
        <span>轨迹文件 ({{ store.project?.tracks.length ?? 0 }})</span>
        <button type="button" aria-label="添加轨迹">
          <IconGlyph name="plus" :size="15" />
        </button>
      </div>
      <article v-if="track" class="track-card">
        <div class="track-card-title">
          <span class="native-checkbox checked">
            <IconGlyph name="check" :size="12" :stroke-width="2.5" />
          </span>
          <IconGlyph name="route" :size="17" />
          <strong>{{ track.name }}</strong>
          <i />
        </div>
        <p>
          {{ track.startUtc.slice(0, 10) }} ·
          {{ new Date(track.startUtc).toISOString().slice(11, 16) }} -
          {{ new Date(track.endUtc).toISOString().slice(11, 16) }}
        </p>
        <dl>
          <div>
            <dt>点数</dt>
            <dd>{{ track.pointCount.toLocaleString("zh-CN") }}</dd>
          </div>
          <div>
            <dt>距离</dt>
            <dd>{{ (track.distanceMeters / 1_000).toFixed(1) }} km</dd>
          </div>
          <div>
            <dt>时长</dt>
            <dd>{{ formatDuration(track.startUtc, track.endUtc) }}</dd>
          </div>
          <div>
            <dt>海拔</dt>
            <dd>{{ track.elevationMin }} - {{ track.elevationMax }} m</dd>
          </div>
        </dl>
        <div class="track-crs">
          <span>{{ track.sourceCrs }}</span>
          <span>→</span>
          <strong>WGS84</strong>
        </div>
      </article>
      <div v-else class="empty-compact">尚未导入轨迹</div>
    </section>

    <section class="photo-section">
      <div class="section-heading photo-heading">
        <span>照片 ({{ store.project?.photos.length ?? 0 }})</span>
        <div>
          <button type="button" title="选择当前筛选" @click="store.selectFilteredPhotos">
            <IconGlyph name="check" :size="15" />
          </button>
          <button type="button" title="清除选择" @click="store.clearPhotoSelection">
            <IconGlyph name="close" :size="15" />
          </button>
        </div>
      </div>

      <div class="photo-controls">
        <label class="search-field">
          <IconGlyph name="search" :size="14" />
          <input v-model="store.searchQuery" placeholder="搜索文件名" />
        </label>
        <label class="filter-select">
          <IconGlyph name="filter" :size="14" />
          <select v-model="store.statusFilter" aria-label="按状态筛选">
            <option
              v-for="option in filterOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </option>
          </select>
        </label>
      </div>

      <div class="photo-list" role="listbox" aria-label="照片列表">
        <button
          v-for="photo in store.filteredPhotos"
          :key="photo.id"
          class="photo-row"
          :class="{ active: store.activePhotoId === photo.id }"
          type="button"
          role="option"
          :aria-selected="store.activePhotoId === photo.id"
          @click="store.selectPhoto(photo.id)"
        >
          <span
            class="native-checkbox"
            :class="{ checked: store.selectedPhotoIds.includes(photo.id) }"
            role="checkbox"
            :aria-checked="store.selectedPhotoIds.includes(photo.id)"
            tabindex="0"
            @click.stop="store.togglePhotoSelection(photo.id)"
            @keydown.space.prevent.stop="store.togglePhotoSelection(photo.id)"
          >
            <IconGlyph
              v-if="store.selectedPhotoIds.includes(photo.id)"
              name="check"
              :size="11"
              :stroke-width="2.6"
            />
          </span>
          <span
            class="photo-thumb"
            :style="{ '--thumb-tone': photo.thumbnailTone ?? '#6f8277' }"
          >
            <img
              v-if="photo.thumbnailUrl"
              :src="photo.thumbnailUrl"
              alt=""
              loading="lazy"
            />
            <i />
          </span>
          <span class="photo-copy">
            <strong>{{ photo.fileName }}</strong>
            <small>{{ formatCapture(photo) }}</small>
            <span
              class="photo-status"
              :class="`status-${statusTone(matchStatus(photo.id))}`"
            >
              <i />
              {{ statusLabel(matchStatus(photo.id)) }}
              <b v-if="photo.existingGps">已有 GPS</b>
            </span>
          </span>
        </button>
        <div v-if="store.filteredPhotos.length === 0" class="empty-compact">
          当前筛选没有照片
        </div>
      </div>

      <footer class="photo-selection-summary">
        已选择 {{ store.selectedPhotoIds.length }} 张 · 可写入
        {{ store.selectedWritableCount }} 张
      </footer>
    </section>
  </aside>
</template>
