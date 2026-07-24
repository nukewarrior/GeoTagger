<script setup lang="ts">
import { computed } from "vue";

import IconGlyph from "@/components/IconGlyph.vue";
import { useWorkspaceStore } from "@/stores/workspace";

const store = useWorkspaceStore();

const recentJobs = computed(() => store.project?.writeHistory ?? []);

function formatTime(value?: string): string {
  if (!value) return "--";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

function announceExport(format: "CSV" | "JSON"): void {
  store.notice = `${format} 报告会在桌面版本中导出到用户选择的位置。`;
}
</script>

<template>
  <main class="tasks-view">
    <header class="page-heading">
      <div>
        <span class="eyebrow">OPERATIONS LOG</span>
        <h1>任务与报告</h1>
        <p>长任务的进度、警告与失败文件会集中保留在这里。</p>
      </div>
      <div class="header-actions">
        <button class="secondary-button" type="button" @click="announceExport('JSON')">
          <IconGlyph name="export" />
          导出 JSON
        </button>
        <button class="primary-button" type="button" @click="announceExport('CSV')">
          <IconGlyph name="export" />
          导出 CSV
        </button>
      </div>
    </header>

    <section class="task-metrics">
      <article>
        <span>照片总数</span>
        <strong>{{ store.statistics.total }}</strong>
        <small>当前项目</small>
      </article>
      <article>
        <span>可写入匹配</span>
        <strong>{{
          store.statistics.high + store.statistics.medium + store.statistics.low
        }}</strong>
        <small>含待人工确认</small>
      </article>
      <article>
        <span>异常</span>
        <strong>{{ store.statistics.failed }}</strong>
        <small>缺时间 / 越界 / 分段空档</small>
      </article>
      <article>
        <span>写入任务</span>
        <strong>{{ recentJobs.length }}</strong>
        <small>结果可追溯</small>
      </article>
    </section>

    <section class="task-layout">
      <div class="task-list-card">
        <div class="card-heading">
          <strong>当前任务</strong>
          <span>{{ store.tasks.length }} 个</span>
        </div>
        <div v-if="store.tasks.length" class="task-list">
          <article v-for="task in store.tasks" :key="task.id">
            <span class="task-icon">
              <IconGlyph :name="task.status === 'FAILED' ? 'warning' : 'tasks'" />
            </span>
            <div>
              <strong>{{ task.label }}</strong>
              <p>{{ task.message }}</p>
              <div class="task-progress">
                <i
                  :style="{
                    width: `${task.total ? (task.completed / task.total) * 100 : 0}%`,
                  }"
                />
              </div>
              <small>
                {{ task.stage }} · {{ task.completed }} / {{ task.total }}
              </small>
            </div>
            <b :class="`task-${task.status.toLocaleLowerCase()}`">
              {{ task.status }}
            </b>
          </article>
        </div>
        <div v-else class="empty-task">
          <IconGlyph name="check" :size="28" />
          <strong>没有正在运行的任务</strong>
          <p>导入、匹配、写入与报告任务会显示在这里。</p>
        </div>
      </div>

      <div class="history-card">
        <div class="card-heading">
          <strong>写入历史</strong>
          <span>最近 {{ recentJobs.length }} 次</span>
        </div>
        <div v-if="recentJobs.length" class="history-list">
          <article v-for="job in recentJobs" :key="job.id">
            <span
              class="history-status"
              :class="{ failed: job.failed > 0 }"
            >
              <IconGlyph :name="job.failed ? 'warning' : 'check'" />
            </span>
            <div>
              <strong>写入并验证照片副本</strong>
              <p>
                成功 {{ job.succeeded }} · 失败 {{ job.failed }}
              </p>
              <small>
                {{ formatTime(job.startedAt) }} - {{ formatTime(job.finishedAt) }}
              </small>
            </div>
            <button type="button">详情</button>
          </article>
        </div>
        <div v-else class="empty-task compact">
          <IconGlyph name="write" :size="25" />
          <strong>暂无写入记录</strong>
          <p>完成一次安全写入后，验证结果会保存在项目中。</p>
        </div>
      </div>
    </section>

    <section class="diagnostic-card">
      <div>
        <IconGlyph name="shield" :size="24" />
        <span>
          <strong>隐私友好的诊断日志</strong>
          <p>默认隐藏完整私人路径与精确坐标；导出诊断包前会再次征得确认。</p>
        </span>
      </div>
      <button class="secondary-button" type="button">导出诊断包…</button>
    </section>
  </main>
</template>
