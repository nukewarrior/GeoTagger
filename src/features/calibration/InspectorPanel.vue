<script setup lang="ts">
import { computed } from "vue";

import IconGlyph from "@/components/IconGlyph.vue";
import { useWorkspaceStore } from "@/stores/workspace";

const store = useWorkspaceStore();

const offsetSeconds = computed({
  get: () => (store.project?.settings.fixedOffsetMs ?? 0) / 1_000,
  set: (value: number) => store.setFixedOffsetSeconds(value),
});

const confidenceDots = computed(() =>
  Math.max(0, Math.min(5, Math.round((store.activeMatch?.confidence ?? 0) * 5))),
);

const matchLabel = computed(() => {
  const status = store.activeMatch?.status;
  if (status === "MATCHED_HIGH") return "高置信匹配";
  if (status === "MATCHED_MEDIUM") return "建议检查";
  if (status === "MATCHED_LOW") return "低置信度";
  if (status === "OUT_OF_RANGE") return "超出轨迹范围";
  if (status === "SEGMENT_GAP") return "位于轨迹中断";
  return "无法匹配";
});

function coordinate(value?: number, positive = "E", negative = "W"): string {
  if (typeof value !== "number") return "--";
  return `${Math.abs(value).toFixed(6)}° ${value >= 0 ? positive : negative}`;
}

function adjustOffset(delta: number): void {
  store.setFixedOffsetSeconds(offsetSeconds.value + delta);
}
</script>

<template>
  <aside class="inspector-panel">
    <section class="inspector-section coordinate-section">
      <div class="section-heading">
        <span>坐标系设置</span>
        <small>保留原始值</small>
      </div>
      <label>
        <span>轨迹原始坐标系</span>
        <select v-model="store.project!.tracks[0].sourceCrs">
          <option value="UNKNOWN">待确认</option>
          <option value="WGS84">WGS84 · 标准 GPS</option>
          <option value="GCJ02">GCJ-02 · 中国互联网地图</option>
          <option value="BD09">BD-09 · 百度地图</option>
        </select>
      </label>
      <label>
        <span>标准化为</span>
        <select disabled>
          <option>WGS84 · EXIF GPS</option>
        </select>
      </label>
      <div
        class="validation-chip"
        :class="{ caution: store.project?.tracks[0].sourceCrs === 'UNKNOWN' }"
      >
        <IconGlyph
          :name="store.project?.tracks[0].sourceCrs === 'UNKNOWN' ? 'warning' : 'check'"
          :size="15"
        />
        {{
          store.project?.tracks[0].sourceCrs === "UNKNOWN"
            ? "匹配前必须确认"
            : "转换预览已准备"
        }}
      </div>
    </section>

    <section class="inspector-section calibration-section">
      <div class="section-heading">
        <span>时间校准</span>
        <small>预览不写盘</small>
      </div>
      <label>
        <span>照片默认时区</span>
        <select v-model="store.project!.settings.photoTimezone">
          <option>Asia/Shanghai</option>
          <option>UTC</option>
          <option>Asia/Tokyo</option>
          <option>Europe/London</option>
          <option>America/Los_Angeles</option>
        </select>
      </label>
      <div class="offset-label">
        <span>加到拍摄时间的校正值</span>
        <strong>{{ offsetSeconds > 0 ? "+" : "" }}{{ offsetSeconds }} 秒</strong>
      </div>
      <div class="offset-control">
        <button type="button" title="减少 10 秒" @click="adjustOffset(-10)">
          -10
        </button>
        <button type="button" title="减少 1 秒" @click="adjustOffset(-1)">
          <IconGlyph name="minus" :size="14" />
        </button>
        <input
          v-model.number="offsetSeconds"
          type="range"
          min="-120"
          max="120"
          step="1"
          aria-label="相机时间偏差"
        />
        <button type="button" title="增加 1 秒" @click="adjustOffset(1)">
          <IconGlyph name="plus" :size="14" />
        </button>
        <button type="button" title="增加 10 秒" @click="adjustOffset(10)">
          +10
        </button>
      </div>
      <div class="calibration-hint">
        <i :class="{ working: store.previewPending }" />
        {{
          store.previewPending
            ? "防抖计算匹配预览…"
            : "固定偏差已应用到匹配预览"
        }}
      </div>
    </section>

    <section class="inspector-section photo-detail">
      <div class="section-heading">
        <span>当前照片</span>
        <small>{{ store.activePhoto ? "1 / " + store.statistics.total : "--" }}</small>
      </div>
      <template v-if="store.activePhoto">
        <div class="active-photo-title">
          <span
            class="detail-thumb"
            :style="{ '--thumb-tone': store.activePhoto.thumbnailTone ?? '#718278' }"
          >
            <i />
          </span>
          <div>
            <strong>{{ store.activePhoto.fileName }}</strong>
            <p>{{ store.activePhoto.relativePath }}</p>
            <span v-if="store.activePhoto.existingGps" class="existing-badge">
              已有 GPS
            </span>
          </div>
        </div>

        <dl class="metadata-grid">
          <dt>拍摄时间</dt>
          <dd>{{ store.activePhoto.captureLocal?.replace("T", " ") ?? "--" }}</dd>
          <dt>标准化 UTC</dt>
          <dd>{{ store.activePhoto.captureUtc?.replace("T", " ").replace("Z", "") ?? "--" }}</dd>
          <dt>前 / 后轨迹点</dt>
          <dd>
            {{ store.activeMatch?.beforeDeltaSeconds ?? "--" }}s /
            {{ store.activeMatch?.afterDeltaSeconds ?? "--" }}s
          </dd>
        </dl>

        <div class="confidence-block">
          <div>
            <span>匹配置信度</span>
            <strong>{{ matchLabel }}</strong>
          </div>
          <div class="confidence-meter">
            <i
              v-for="index in 5"
              :key="index"
              :class="{ filled: index <= confidenceDots }"
            />
            <span>{{ Math.round((store.activeMatch?.confidence ?? 0) * 100) }}%</span>
          </div>
          <small v-if="store.activeMatch?.estimatedErrorMeters">
            启发式误差估算约 {{ store.activeMatch.estimatedErrorMeters }} m
          </small>
          <p v-if="store.activeMatch?.reason">{{ store.activeMatch.reason }}</p>
        </div>

        <div class="coordinate-readout detail">
          <span>位置 · WGS84</span>
          <dl>
            <dt>经度</dt>
            <dd>{{ coordinate(store.activeMatch?.lon, "E", "W") }}</dd>
            <dt>纬度</dt>
            <dd>{{ coordinate(store.activeMatch?.lat, "N", "S") }}</dd>
            <dt>高度</dt>
            <dd>
              {{
                store.activeMatch?.elevation !== undefined
                  ? `${store.activeMatch.elevation.toFixed(1)} m`
                  : "--"
              }}
            </dd>
          </dl>
        </div>
      </template>
      <div v-else class="empty-compact">请选择一张照片</div>
    </section>

    <section class="write-actions">
      <button
        class="primary-button full"
        type="button"
        :disabled="!store.canBuildWritePlan"
        @click="store.prepareWritePlan"
      >
        <IconGlyph name="write" />
        写入 EXIF 到副本…
      </button>
      <button class="secondary-button full" type="button" @click="store.setView('tasks')">
        <IconGlyph name="export" />
        导出匹配报告
      </button>
      <p v-if="!store.canBuildWritePlan">
        请选择至少一张已匹配照片并设置输出目录。
      </p>
    </section>
  </aside>
</template>
