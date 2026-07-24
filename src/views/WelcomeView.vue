<script setup lang="ts">
import { ref } from "vue";

import IconGlyph from "@/components/IconGlyph.vue";
import { chooseProjectDirectory, isTauriRuntime } from "@/services/backend";
import { useWorkspaceStore } from "@/stores/workspace";

const store = useWorkspaceStore();
const showCreate = ref(false);
const projectName = ref("我的照片旅程");
const projectDirectory = ref("");

async function selectDirectory(): Promise<void> {
  const selected = await chooseProjectDirectory();
  if (selected) projectDirectory.value = selected;
}

async function submitProject(): Promise<void> {
  if (!isTauriRuntime()) {
    store.loadDemo();
    showCreate.value = false;
    return;
  }

  if (!projectDirectory.value.trim() || !projectName.value.trim()) return;
  await store.startNewProject(
    projectName.value.trim(),
    projectDirectory.value.trim(),
  );
  showCreate.value = false;
}
</script>

<template>
  <main class="welcome-view">
    <section class="welcome-hero">
      <div class="hero-kicker">
        <IconGlyph name="shield" :size="16" />
        <span>照片、轨迹与坐标始终留在本机</span>
      </div>
      <p class="hero-index">FIELD NOTE / 001</p>
      <h1>
        让每一张照片
        <em>回到拍摄的地方</em>
      </h1>
      <p class="hero-copy">
        导入相机照片与 GPX 轨迹，校准时间，检查地图位置，再将 GPS
        安全写入照片副本。原片默认只读，结果可追溯。
      </p>
      <div class="hero-actions">
        <button class="primary-button large" type="button" @click="showCreate = true">
          <IconGlyph name="plus" />
          新建项目
        </button>
        <button class="secondary-button large" type="button" @click="store.openExistingProject">
          <IconGlyph name="folder" />
          打开项目
        </button>
        <button class="text-button" type="button" @click="store.loadDemo">
          <IconGlyph name="play" />
          体验离线演示
        </button>
      </div>
    </section>

    <aside class="welcome-aside" aria-label="工作流程">
      <div class="contour-card">
        <div class="contour-lines" />
        <div class="route-preview">
          <span class="route-node node-a" />
          <span class="route-node node-b" />
          <span class="route-node node-c" />
          <span class="route-photo">
            <IconGlyph name="photo" :size="17" />
          </span>
        </div>
        <div class="coordinate-readout">
          <span>WGS84</span>
          <strong>48.684321° N</strong>
          <strong>86.873456° E</strong>
          <small>EST. 1,392.7 M</small>
        </div>
      </div>
      <ol class="workflow-list">
        <li>
          <span>01</span>
          <div>
            <strong>汇入</strong>
            <p>GPX 轨迹与照片元数据</p>
          </div>
        </li>
        <li>
          <span>02</span>
          <div>
            <strong>校准</strong>
            <p>坐标系、时区与相机偏差</p>
          </div>
        </li>
        <li>
          <span>03</span>
          <div>
            <strong>核验</strong>
            <p>地图、时间轴与置信度</p>
          </div>
        </li>
        <li>
          <span>04</span>
          <div>
            <strong>写入</strong>
            <p>复制、更新 EXIF、重新读取</p>
          </div>
        </li>
      </ol>
    </aside>

    <Transition name="fade">
      <div v-if="showCreate" class="modal-scrim" @click.self="showCreate = false">
        <form class="modal-card" @submit.prevent="submitProject">
          <button
            class="modal-close"
            type="button"
            aria-label="关闭"
            @click="showCreate = false"
          >
            <IconGlyph name="close" />
          </button>
          <span class="eyebrow">NEW FIELD PROJECT</span>
          <h2>创建一个本地项目</h2>
          <p>项目文件会保存导入记录、匹配结果与写入历史，不包含照片内容。</p>
          <label>
            <span>项目名称</span>
            <input v-model="projectName" autofocus required />
          </label>
          <label>
            <span>项目目录</span>
            <div class="input-action">
              <input
                v-model="projectDirectory"
                :placeholder="isTauriRuntime() ? '选择一个可写目录' : '浏览器演示无需目录'"
                :required="isTauriRuntime()"
                readonly
              />
              <button type="button" @click="selectDirectory">选择…</button>
            </div>
          </label>
          <div class="privacy-note">
            <IconGlyph name="shield" :size="18" />
            <span>默认只读取原照片；后续写入仅发生在单独的输出目录。</span>
          </div>
          <button class="primary-button full" type="submit">
            创建并开始导入
          </button>
        </form>
      </div>
    </Transition>
  </main>
</template>
