<script setup lang="ts">
import { computed, ref } from "vue";

import IconGlyph from "@/components/IconGlyph.vue";
import {
  calculateMatches,
  choosePhotoDirectory,
  chooseProjectDirectory,
  chooseTrackFiles,
  importTracks,
  isTauriRuntime,
  scanPhotos,
} from "@/services/backend";
import { useWorkspaceStore } from "@/stores/workspace";
import type { CoordinateSystem } from "@/types/domain";

const store = useWorkspaceStore();
const step = ref(1);
const sourceCrs = ref<CoordinateSystem>("UNKNOWN");
const trackPaths = ref<string[]>([]);
const photoDirectory = ref("");
const busyMessage = ref("");
const importWarning = ref("");

const canContinue = computed(() => {
  if (step.value === 1) return trackPaths.value.length > 0;
  if (step.value === 2) return sourceCrs.value !== "UNKNOWN";
  if (step.value === 3) return Boolean(photoDirectory.value);
  return Boolean(store.project?.settings.outputDirectory);
});

async function selectTracks(): Promise<void> {
  if (!isTauriRuntime()) {
    store.loadDemo();
    step.value = 2;
    trackPaths.value = ["/Demo/DAY3_喀纳斯环线.gpx"];
    return;
  }
  trackPaths.value = await chooseTrackFiles();
}

async function selectPhotos(): Promise<void> {
  if (!isTauriRuntime()) {
    photoDirectory.value = "/Demo/Photos/2024/1003";
    return;
  }
  photoDirectory.value = (await choosePhotoDirectory()) ?? "";
}

async function selectOutput(): Promise<void> {
  const selected = isTauriRuntime()
    ? await chooseProjectDirectory()
    : "/Demo/Output";
  if (selected && store.project) {
    store.project.settings.outputDirectory = selected;
    store.project.dirty = true;
  }
}

async function continueStep(): Promise<void> {
  if (!store.project || !canContinue.value) return;

  if (step.value === 2 && isTauriRuntime()) {
    busyMessage.value = "正在解析、校验并标准化轨迹…";
    try {
      const result = await importTracks(trackPaths.value, sourceCrs.value);
      store.project.tracks = result.tracks;
      importWarning.value = result.warnings.join("；");
    } finally {
      busyMessage.value = "";
    }
  }

  if (step.value === 3 && isTauriRuntime()) {
    busyMessage.value = "正在扫描照片并批量读取元数据…";
    try {
      const result = await scanPhotos(photoDirectory.value, true);
      if (result.photos) store.project.photos = result.photos;
      if (result.taskId) {
        importWarning.value = "照片扫描已进入任务中心，可继续设置输出目录。";
      }
    } finally {
      busyMessage.value = "";
    }
  }

  if (step.value < 4) {
    step.value += 1;
  } else {
    if (store.project.tracks.length === 0 && !isTauriRuntime()) {
      store.loadDemo();
      return;
    }
    if (
      isTauriRuntime() &&
      store.project.tracks.length > 0 &&
      store.project.photos.length > 0
    ) {
      busyMessage.value = "正在按时间建立第一版匹配预览…";
      try {
        const result = await calculateMatches({
          trackIds: store.project.tracks.map((track) => track.id),
          photoIds: store.project.photos.map((photo) => photo.id),
          timezone: store.project.settings.photoTimezone,
          fixedOffsetMs: store.project.settings.fixedOffsetMs,
        });
        store.project.matches = result.matches;
      } catch {
        importWarning.value =
          "初始匹配未完成。可进入工作台检查照片时间后重新计算。";
      } finally {
        busyMessage.value = "";
      }
    }
    store.setView("workspace");
  }
}
</script>

<template>
  <main class="import-view">
    <aside class="import-sidebar">
      <span class="eyebrow">PROJECT INTAKE</span>
      <h1>导入与标准化</h1>
      <p>先确认数据来源，再开始匹配。每一步都可以稍后返回调整。</p>
      <ol class="step-list">
        <li
          v-for="(item, index) in [
            ['轨迹文件', '选择一个或多个 GPX'],
            ['原始坐标系', '确认转换为 WGS84'],
            ['照片目录', '读取时间与已有 GPS'],
            ['输出策略', '只写入独立副本'],
          ]"
          :key="item[0]"
          :class="{ active: step === index + 1, done: step > index + 1 }"
        >
          <span>{{ String(index + 1).padStart(2, "0") }}</span>
          <div>
            <strong>{{ item[0] }}</strong>
            <p>{{ item[1] }}</p>
          </div>
        </li>
      </ol>
      <div class="import-safety">
        <IconGlyph name="shield" />
        <p>原照片在整个导入和预览阶段保持只读。</p>
      </div>
    </aside>

    <section class="import-stage">
      <div class="stage-progress">
        <span>步骤 {{ step }} / 4</span>
        <div><i :style="{ width: `${step * 25}%` }" /></div>
      </div>

      <div v-if="step === 1" class="stage-card">
        <IconGlyph class="stage-icon" name="route" :size="34" />
        <span class="eyebrow">TRACK SOURCES</span>
        <h2>选择 GPX 轨迹</h2>
        <p>
          支持多个文件与多个轨迹分段。分段边界会被保留，匹配不会跨越轨迹中断。
        </p>
        <button class="drop-zone" type="button" @click="selectTracks">
          <IconGlyph name="plus" :size="28" />
          <strong>选择 GPX 文件</strong>
          <span>仅在本机解析，不上传</span>
        </button>
        <ul v-if="trackPaths.length" class="selected-files">
          <li v-for="path in trackPaths" :key="path">
            <IconGlyph name="route" />
            <span>{{ path.split('/').at(-1) }}</span>
            <small>{{ path }}</small>
            <IconGlyph name="check" />
          </li>
        </ul>
      </div>

      <div v-else-if="step === 2" class="stage-card">
        <IconGlyph class="stage-icon" name="transform" :size="34" />
        <span class="eyebrow">COORDINATE REFERENCE</span>
        <h2>确认轨迹原始坐标系</h2>
        <p>GPX 本身通常无法可靠声明中国偏移坐标，请根据轨迹来源人工确认。</p>
        <div class="crs-options">
          <label
            v-for="option in [
              ['WGS84', '标准 GPS / 大多数运动设备'],
              ['GCJ02', '高德、腾讯等中国互联网地图'],
              ['BD09', '百度地图导出的坐标'],
            ]"
            :key="option[0]"
            :class="{ selected: sourceCrs === option[0] }"
          >
            <input
              v-model="sourceCrs"
              type="radio"
              name="crs"
              :value="option[0]"
            />
            <span class="crs-code">{{ option[0] }}</span>
            <span>
              <strong>{{ option[1] }}</strong>
              <small>内部将保存原始值与 WGS84 转换记录</small>
            </span>
            <IconGlyph v-if="sourceCrs === option[0]" name="check" />
          </label>
        </div>
      </div>

      <div v-else-if="step === 3" class="stage-card">
        <IconGlyph class="stage-icon" name="photo" :size="34" />
        <span class="eyebrow">PHOTO LIBRARY</span>
        <h2>选择照片目录</h2>
        <p>
          将批量读取拍摄时间、时区与已有 GPS。JPEG/TIFF 可在后续复制写入，RAW
          原件保持只读。
        </p>
        <button class="drop-zone" type="button" @click="selectPhotos">
          <IconGlyph name="folder" :size="30" />
          <strong>{{ photoDirectory || "选择照片文件夹" }}</strong>
          <span>可递归扫描子目录</span>
        </button>
        <div class="timezone-row">
          <label>
            <span>照片缺少时区时使用</span>
            <select v-model="store.project!.settings.photoTimezone">
              <option>Asia/Shanghai</option>
              <option>UTC</option>
              <option>Asia/Tokyo</option>
              <option>Europe/London</option>
              <option>America/Los_Angeles</option>
            </select>
          </label>
          <p>带 OffsetTimeOriginal 的照片会优先使用自身时区。</p>
        </div>
      </div>

      <div v-else class="stage-card">
        <IconGlyph class="stage-icon" name="shield" :size="34" />
        <span class="eyebrow">SAFE OUTPUT</span>
        <h2>设置独立输出目录</h2>
        <p>
          GeoTagger 默认先复制文件，再写入并重新读取验证。不会静默覆盖任何源照片。
        </p>
        <button class="output-choice" type="button" @click="selectOutput">
          <IconGlyph name="folder" />
          <span>
            <strong>{{
              store.project?.settings.outputDirectory || "选择输出目录"
            }}</strong>
            <small>保留原始相对目录结构，冲突会在写入计划中列出</small>
          </span>
          <span>选择…</span>
        </button>
        <div class="policy-card">
          <div>
            <IconGlyph name="check" />
            <span>
              <strong>复制后写入</strong>
              <small>原照片只读；输出使用临时文件与原子重命名</small>
            </span>
          </div>
          <label>
            已有 GPS
            <select v-model="store.project!.settings.existingGpsPolicy">
              <option value="SKIP">跳过</option>
              <option value="OVERWRITE">经确认后覆盖副本</option>
              <option value="PRESERVE_COPY">保留旧值，仅复制</option>
            </select>
          </label>
        </div>
      </div>

      <div v-if="importWarning" class="inline-warning">
        <IconGlyph name="warning" />
        {{ importWarning }}
      </div>

      <div class="stage-actions">
        <button
          class="secondary-button"
          type="button"
          :disabled="step === 1"
          @click="step -= 1"
        >
          上一步
        </button>
        <span v-if="busyMessage" class="busy-label">{{ busyMessage }}</span>
        <button
          class="primary-button"
          type="button"
          :disabled="!canContinue || Boolean(busyMessage)"
          @click="continueStep"
        >
          {{ step === 4 ? "进入匹配工作台" : "继续" }}
        </button>
      </div>
    </section>
  </main>
</template>
