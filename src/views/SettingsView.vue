<script setup lang="ts">
import { computed, ref } from "vue";

import IconGlyph from "@/components/IconGlyph.vue";
import { useWorkspaceStore } from "@/stores/workspace";

const store = useWorkspaceStore();
const cacheSize = ref("0 B");
const mapStyle = ref("");
const privacyDiagnostics = ref(false);
const defaultTimezone = ref("Asia/Shanghai");
const standaloneUpdates = ref(false);

const timezone = computed({
  get: () => store.project?.settings.photoTimezone ?? defaultTimezone.value,
  set: (value: string) => {
    defaultTimezone.value = value;
    if (store.project) store.project.settings.photoTimezone = value;
  },
});

const automaticUpdates = computed({
  get: () =>
    store.project?.settings.automaticUpdates ?? standaloneUpdates.value,
  set: (value: boolean) => {
    standaloneUpdates.value = value;
    if (store.project) store.project.settings.automaticUpdates = value;
  },
});

function clearCache(): void {
  cacheSize.value = "0 B";
  store.notice = "缩略图、地图与临时轨迹缓存已清理。";
}
</script>

<template>
  <main class="settings-view">
    <header class="page-heading">
      <div>
        <span class="eyebrow">APPLICATION PREFERENCES</span>
        <h1>设置</h1>
        <p>地图、ExifTool、缓存、默认时区与隐私控制。</p>
      </div>
      <button
        v-if="store.project"
        class="secondary-button"
        type="button"
        @click="store.setView('workspace')"
      >
        返回工作台
      </button>
    </header>

    <section class="settings-grid">
      <article class="settings-card">
        <div class="settings-card-title">
          <span><IconGlyph name="map" /></span>
          <div>
            <strong>地图与离线显示</strong>
            <p>MapLibre 引擎随应用打包，轨迹与照片图层始终离线可见。</p>
          </div>
        </div>
        <label>
          <span>
            <strong>地图样式地址</strong>
            <small>留空时使用内置离线画布，不请求网络</small>
          </span>
          <input v-model="mapStyle" placeholder="本地文件或自有 HTTPS style.json" />
        </label>
        <div class="setting-row">
          <span>
            <strong>地图提供方</strong>
            <small>高德模式不在 MVP 中启用</small>
          </span>
          <select disabled>
            <option>MapLibre · WGS84</option>
          </select>
        </div>
      </article>

      <article class="settings-card">
        <div class="settings-card-title">
          <span><IconGlyph name="write" /></span>
          <div>
            <strong>ExifTool Sidecar</strong>
            <p>固定版本由 GitHub Actions 下载、校验并打入对应平台安装包。</p>
          </div>
        </div>
        <div class="health-row">
          <span class="health-dot" />
          <span>
            <strong>ExifTool 13.59</strong>
            <small>桌面启动时执行版本检查与最小读写自检</small>
          </span>
          <b>随包提供</b>
        </div>
        <div class="setting-note">
          前端不会拼接 Shell 命令；所有路径与参数由 Rust 校验后以参数数组调用。
        </div>
      </article>

      <article class="settings-card">
        <div class="settings-card-title">
          <span><IconGlyph name="clock" /></span>
          <div>
            <strong>时间与匹配默认值</strong>
            <p>仅在照片没有自带时区时使用项目默认时区。</p>
          </div>
        </div>
        <div class="setting-row">
          <span>
            <strong>默认照片时区</strong>
            <small>新项目的初始值</small>
          </span>
          <select v-model="timezone">
            <option>Asia/Shanghai</option>
            <option>UTC</option>
            <option>Asia/Tokyo</option>
            <option>Europe/London</option>
            <option>America/Los_Angeles</option>
          </select>
        </div>
        <div class="setting-row">
          <span>
            <strong>超过 60 秒的插值</strong>
            <small>始终标为低置信度并等待人工确认</small>
          </span>
          <b>保守模式</b>
        </div>
      </article>

      <article class="settings-card">
        <div class="settings-card-title">
          <span><IconGlyph name="shield" /></span>
          <div>
            <strong>隐私与诊断</strong>
            <p>不上传照片、轨迹、缩略图或坐标；在线地图请求除外。</p>
          </div>
        </div>
        <label class="toggle-row">
          <span>
            <strong>诊断包包含项目摘要</strong>
            <small>始终需要在每次导出前明确确认</small>
          </span>
          <input v-model="privacyDiagnostics" type="checkbox" />
        </label>
        <label class="toggle-row">
          <span>
            <strong>自动检查更新</strong>
            <small>可关闭；安装前展示版本、变更与校验信息</small>
          </span>
          <input v-model="automaticUpdates" type="checkbox" />
        </label>
      </article>

      <article class="settings-card wide">
        <div class="settings-card-title">
          <span><IconGlyph name="layers" /></span>
          <div>
            <strong>本地缓存</strong>
            <p>缩略图、地图缓存和临时标准化轨迹可以随时安全清理。</p>
          </div>
        </div>
        <div class="cache-row">
          <div>
            <span class="cache-bar"><i style="width: 4%" /></span>
            <small>当前缓存 {{ cacheSize }}</small>
          </div>
          <button class="secondary-button" type="button" @click="clearCache">
            清理缓存
          </button>
        </div>
      </article>
    </section>
  </main>
</template>
