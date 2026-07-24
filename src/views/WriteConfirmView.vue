<script setup lang="ts">
import { computed, ref } from "vue";

import IconGlyph from "@/components/IconGlyph.vue";
import { executeWritePlan, isTauriRuntime } from "@/services/backend";
import { useWorkspaceStore } from "@/stores/workspace";

const store = useWorkspaceStore();
const acknowledged = ref(false);
const executing = ref(false);
const completed = ref(false);
const failedCount = ref(0);

const writeItems = computed(
  () => store.writePlan?.items.filter((item) => item.action === "WRITE") ?? [],
);
const skippedItems = computed(
  () => store.writePlan?.items.filter((item) => item.action === "SKIP") ?? [],
);

async function execute(): Promise<void> {
  if (!store.writePlan || !acknowledged.value) return;
  executing.value = true;

  try {
    if (isTauriRuntime()) {
      const result = await executeWritePlan(store.writePlan.id);
      failedCount.value = result.failedPhotoIds.length;
      store.project?.writeHistory.unshift(result.job);
    } else {
      await new Promise((resolve) => setTimeout(resolve, 900));
      failedCount.value = 0;
      store.project?.writeHistory.unshift({
        id: "demo-write-job",
        writePlanId: store.writePlan.id,
        status: "COMPLETED",
        startedAt: new Date(Date.now() - 900).toISOString(),
        finishedAt: new Date().toISOString(),
        succeeded: writeItems.value.length,
        failed: 0,
      });
    }
    completed.value = true;
  } catch {
    store.error = {
      code: "WRITE_VERIFY_FAILED",
      message: "部分照片写入或回读验证失败。",
      suggestion: "源照片未被修改；请在任务中心查看失败文件并单独重试。",
    };
  } finally {
    executing.value = false;
  }
}

function compactPath(path: string): string {
  const segments = path.split("/");
  return segments.length > 4 ? `…/${segments.slice(-3).join("/")}` : path;
}
</script>

<template>
  <main class="write-confirm-view">
    <header class="page-heading">
      <div>
        <span class="eyebrow">WRITE PLAN / IMMUTABLE PREVIEW</span>
        <h1>确认写入计划</h1>
        <p>所有操作都针对新副本；每个输出文件写入后会立即重新读取验证。</p>
      </div>
      <button class="secondary-button" type="button" @click="store.setView('workspace')">
        返回工作台
      </button>
    </header>

    <template v-if="store.writePlan && !completed">
      <section class="write-overview">
        <article class="overview-card accent-card">
          <span>计划写入</span>
          <strong>{{ writeItems.length }}</strong>
          <small>张已匹配照片</small>
        </article>
        <article class="overview-card">
          <span>自动跳过</span>
          <strong>{{ skippedItems.length }}</strong>
          <small>无位置或已有 GPS</small>
        </article>
        <article class="overview-card">
          <span>源文件状态</span>
          <strong class="word">
            {{ store.writePlan.sourceFilesUnchanged ? "未变化" : "已变化" }}
          </strong>
          <small>哈希、大小与修改时间</small>
        </article>
        <article class="overview-card">
          <span>写入策略</span>
          <strong class="word">副本</strong>
          <small>临时文件 + 原子重命名</small>
        </article>
      </section>

      <section class="write-layout">
        <div class="write-table-card">
          <div class="card-heading">
            <div>
              <strong>文件变更明细</strong>
              <span>{{ store.writePlan.items.length }} 个项目</span>
            </div>
            <span class="plan-id">{{ store.writePlan.id }}</span>
          </div>
          <div class="write-table">
            <div class="write-row table-head">
              <span>照片</span>
              <span>原 GPS</span>
              <span>新 GPS · WGS84</span>
              <span>操作</span>
            </div>
            <div
              v-for="item in store.writePlan.items"
              :key="item.photoId"
              class="write-row"
              :class="{ skipped: item.action === 'SKIP' }"
            >
              <span class="write-file">
                <IconGlyph name="photo" :size="17" />
                <span>
                  <strong>{{ item.sourcePath.split("/").at(-1) }}</strong>
                  <small>{{ compactPath(item.outputPath) }}</small>
                </span>
              </span>
              <span>
                {{
                  item.oldGps
                    ? `${item.oldGps.lat.toFixed(5)}, ${item.oldGps.lon.toFixed(5)}`
                    : "无"
                }}
              </span>
              <span class="new-coordinate">
                {{
                  item.newGps
                    ? `${item.newGps.lat.toFixed(6)}, ${item.newGps.lon.toFixed(6)}`
                    : "--"
                }}
                <small v-if="item.newGps?.altitude !== undefined">
                  {{ item.newGps.altitude.toFixed(1) }} m
                </small>
              </span>
              <span>
                <b :class="item.action === 'WRITE' ? 'action-write' : 'action-skip'">
                  {{ item.action === "WRITE" ? "复制并写入" : "跳过" }}
                </b>
                <small v-if="item.reason">{{ item.reason }}</small>
              </span>
            </div>
          </div>
        </div>

        <aside class="write-checklist">
          <div class="card-heading">
            <strong>安全检查</strong>
          </div>
          <ul>
            <li :class="{ pass: store.writePlan.sourceFilesUnchanged }">
              <IconGlyph
                :name="store.writePlan.sourceFilesUnchanged ? 'check' : 'warning'"
              />
              <span>
                <strong>源文件未发生变化</strong>
                <small>已比对哈希、大小和修改时间</small>
              </span>
            </li>
            <li class="pass">
              <IconGlyph name="check" />
              <span>
                <strong>输出目录独立</strong>
                <small>{{ compactPath(store.writePlan.outputDirectory) }}</small>
              </span>
            </li>
            <li class="pass">
              <IconGlyph name="check" />
              <span>
                <strong>只写入支持格式</strong>
                <small>JPEG / TIFF；RAW 原件自动跳过</small>
              </span>
            </li>
            <li :class="{ pass: store.writePlan.warnings.length === 0, caution: store.writePlan.warnings.length }">
              <IconGlyph
                :name="store.writePlan.warnings.length ? 'warning' : 'check'"
              />
              <span>
                <strong>
                  {{
                    store.writePlan.warnings.length
                      ? `${store.writePlan.warnings.length} 项需要注意`
                      : "没有阻塞风险"
                  }}
                </strong>
                <small>{{
                  store.writePlan.warnings[0] ?? "可以安全开始写入"
                }}</small>
              </span>
            </li>
          </ul>
          <label class="acknowledge-row">
            <input v-model="acknowledged" type="checkbox" />
            <span>
              我已检查新坐标与输出目录，确认只写入副本。
            </span>
          </label>
          <button
            class="primary-button full write-now"
            type="button"
            :disabled="
              !acknowledged ||
              executing ||
              !store.writePlan.sourceFilesUnchanged ||
              writeItems.length === 0
            "
            @click="execute"
          >
            <IconGlyph :name="executing ? 'clock' : 'write'" />
            {{ executing ? "复制、写入并验证中…" : `写入 ${writeItems.length} 个副本` }}
          </button>
          <p>任务可取消；失败不会影响其他文件或源照片。</p>
        </aside>
      </section>
    </template>

    <section v-else-if="completed" class="write-complete">
      <span class="complete-mark">
        <IconGlyph name="check" :size="42" :stroke-width="1.6" />
      </span>
      <span class="eyebrow">WRITE & VERIFY COMPLETE</span>
      <h2>{{ writeItems.length - failedCount }} 张照片已验证</h2>
      <p v-if="failedCount">
        {{ failedCount }} 个文件需要重试。源文件保持不变，详细日志已保留。
      </p>
      <p v-else>
        所有新副本的 GPS 均已重新读取并与写入计划一致。
      </p>
      <div>
        <button class="primary-button" type="button" @click="store.setView('workspace')">
          返回工作台
        </button>
        <button class="secondary-button" type="button" @click="store.setView('tasks')">
          查看任务与报告
        </button>
      </div>
    </section>

    <section v-else class="empty-page">
      <IconGlyph name="write" :size="36" />
      <h2>尚未生成写入计划</h2>
      <p>回到工作台选择照片并确认输出目录。</p>
      <button class="primary-button" type="button" @click="store.setView('workspace')">
        返回工作台
      </button>
    </section>
  </main>
</template>
