import { computed, ref } from "vue";
import { defineStore } from "pinia";

import { createDemoProject } from "@/data/demoProject";
import {
  buildWritePlan as requestWritePlan,
  calculateMatches,
  chooseProjectFile,
  createProject,
  isTauriRuntime,
  openProject,
  saveProject,
} from "@/services/backend";
import type {
  AppError,
  AppView,
  MatchStatus,
  PhotoMatch,
  ProjectSnapshot,
  TaskInfo,
  WritePlan,
} from "@/types/domain";

const MATCHED_STATUSES: MatchStatus[] = [
  "MATCHED_HIGH",
  "MATCHED_MEDIUM",
  "MATCHED_LOW",
];

let calibrationTimer: ReturnType<typeof setTimeout> | undefined;

export const useWorkspaceStore = defineStore("workspace", () => {
  const project = ref<ProjectSnapshot>();
  const activeView = ref<AppView>("welcome");
  const activePhotoId = ref<string>();
  const selectedPhotoIds = ref<string[]>([]);
  const statusFilter = ref<MatchStatus | "ALL">("ALL");
  const searchQuery = ref("");
  const writePlan = ref<WritePlan>();
  const tasks = ref<TaskInfo[]>([]);
  const loading = ref(false);
  const previewPending = ref(false);
  const notice = ref<string>();
  const error = ref<AppError>();
  const mapReady = ref(false);
  const mapBaseAvailable = ref(false);

  const matchesByPhotoId = computed(
    () =>
      new Map(
        (project.value?.matches ?? []).map((match) => [match.photoId, match]),
      ),
  );

  const activePhoto = computed(() =>
    project.value?.photos.find((photo) => photo.id === activePhotoId.value),
  );

  const activeMatch = computed(() =>
    activePhotoId.value
      ? matchesByPhotoId.value.get(activePhotoId.value)
      : undefined,
  );

  const filteredPhotos = computed(() => {
    const query = searchQuery.value.trim().toLocaleLowerCase();

    return (project.value?.photos ?? []).filter((photo) => {
      const match = matchesByPhotoId.value.get(photo.id);
      const matchesStatus =
        statusFilter.value === "ALL" || match?.status === statusFilter.value;
      const matchesQuery =
        query.length === 0 ||
        photo.fileName.toLocaleLowerCase().includes(query) ||
        photo.relativePath.toLocaleLowerCase().includes(query);

      return matchesStatus && matchesQuery;
    });
  });

  const statistics = computed(() => {
    const result = {
      total: project.value?.photos.length ?? 0,
      high: 0,
      medium: 0,
      low: 0,
      failed: 0,
      existingGps: 0,
    };

    for (const photo of project.value?.photos ?? []) {
      const status = matchesByPhotoId.value.get(photo.id)?.status;
      if (status === "MATCHED_HIGH") result.high += 1;
      else if (status === "MATCHED_MEDIUM") result.medium += 1;
      else if (status === "MATCHED_LOW") result.low += 1;
      else result.failed += 1;
      if (photo.existingGps) result.existingGps += 1;
    }

    return result;
  });

  const selectedWritableCount = computed(
    () =>
      selectedPhotoIds.value.filter((photoId) => {
        const match = matchesByPhotoId.value.get(photoId);
        const photo = project.value?.photos.find((item) => item.id === photoId);
        if (!match || !MATCHED_STATUSES.includes(match.status)) return false;
        if (
          photo?.existingGps &&
          project.value?.settings.existingGpsPolicy === "SKIP"
        ) {
          return false;
        }
        return true;
      }).length,
  );

  const canBuildWritePlan = computed(
    () =>
      selectedWritableCount.value > 0 &&
      Boolean(project.value?.settings.outputDirectory) &&
      !previewPending.value,
  );

  function setProject(snapshot: ProjectSnapshot): void {
    project.value = snapshot;
    activePhotoId.value = snapshot.photos[0]?.id;
    selectedPhotoIds.value = snapshot.photos
      .filter((photo) => {
        const status = snapshot.matches.find(
          (match) => match.photoId === photo.id,
        )?.status;
        return status ? MATCHED_STATUSES.includes(status) : false;
      })
      .map((photo) => photo.id);
    activeView.value = snapshot.tracks.length > 0 ? "workspace" : "import";
    notice.value = undefined;
    error.value = undefined;
  }

  function loadDemo(): void {
    setProject(createDemoProject());
    notice.value = "已载入本地演示项目；所有数据均为离线示例。";
  }

  async function startNewProject(name: string, directory: string): Promise<void> {
    loading.value = true;
    error.value = undefined;
    try {
      setProject(await createProject({ name, directory }));
      activeView.value = "import";
    } catch {
      error.value = {
        code: "PROJECT_INVALID",
        message: "项目创建失败。",
        suggestion: "请确认目录可写，且项目名称不包含无效字符。",
      };
    } finally {
      loading.value = false;
    }
  }

  async function openExistingProject(): Promise<void> {
    if (!isTauriRuntime()) {
      loadDemo();
      return;
    }

    const projectPath = await chooseProjectFile();
    if (!projectPath) return;
    loading.value = true;
    error.value = undefined;

    try {
      setProject(await openProject(projectPath));
    } catch {
      error.value = {
        code: "PROJECT_INVALID",
        message: "无法打开这个项目文件。",
        suggestion: "检查文件是否完整，或从同目录的恢复副本重新打开。",
      };
    } finally {
      loading.value = false;
    }
  }

  async function persistProject(): Promise<void> {
    if (!project.value) return;

    loading.value = true;
    try {
      project.value = await saveProject(project.value);
      notice.value = "项目已安全保存。";
    } catch {
      error.value = {
        code: "PROJECT_INVALID",
        message: "项目保存失败。",
        suggestion: "请检查项目目录权限，然后重试。",
      };
    } finally {
      loading.value = false;
    }
  }

  function selectPhoto(photoId: string): void {
    activePhotoId.value = photoId;
  }

  function togglePhotoSelection(photoId: string): void {
    selectedPhotoIds.value = selectedPhotoIds.value.includes(photoId)
      ? selectedPhotoIds.value.filter((id) => id !== photoId)
      : [...selectedPhotoIds.value, photoId];
  }

  function selectFilteredPhotos(): void {
    const filteredIds = filteredPhotos.value.map((photo) => photo.id);
    selectedPhotoIds.value = Array.from(
      new Set([...selectedPhotoIds.value, ...filteredIds]),
    );
  }

  function clearPhotoSelection(): void {
    selectedPhotoIds.value = [];
  }

  function updateDemoMatches(offsetMs: number): void {
    if (!project.value) return;
    const normalizedOffset = Math.abs(offsetMs / 1_000 + 12);

    project.value.matches = project.value.matches.map((match, index) => {
      if (!MATCHED_STATUSES.includes(match.status)) return match;
      const penalty = Math.min(0.48, normalizedOffset * 0.018);
      const confidence = Math.max(0.2, match.confidence - penalty);
      const status: MatchStatus =
        confidence >= 0.78
          ? "MATCHED_HIGH"
          : confidence >= 0.52
            ? "MATCHED_MEDIUM"
            : "MATCHED_LOW";

      return {
        ...match,
        status,
        confidence,
        beforeDeltaSeconds: 2 + (index % 4) + Math.round(normalizedOffset),
        afterDeltaSeconds: 3 + (index % 5),
        estimatedErrorMeters: Math.round((1 - confidence) * 70 + 3),
      };
    });
  }

  function setFixedOffsetSeconds(seconds: number): void {
    if (!project.value) return;
    project.value.settings.fixedOffsetMs = Math.round(seconds * 1_000);
    project.value.dirty = true;
    previewPending.value = true;

    if (calibrationTimer) clearTimeout(calibrationTimer);
    calibrationTimer = setTimeout(async () => {
      try {
        if (!project.value) return;
        if (isTauriRuntime()) {
          const result = await calculateMatches({
            trackIds: project.value.tracks.map((track) => track.id),
            photoIds: project.value.photos.map((photo) => photo.id),
            timezone: project.value.settings.photoTimezone,
            fixedOffsetMs: project.value.settings.fixedOffsetMs,
          });
          project.value.matches = result.matches;
        } else {
          updateDemoMatches(project.value.settings.fixedOffsetMs);
        }
      } catch {
        error.value = {
          code: "MATCH_NOT_FOUND",
          message: "时间校准预览计算失败。",
          suggestion: "确认轨迹含时间，并检查照片时区后重试。",
        };
      } finally {
        previewPending.value = false;
      }
    }, 280);
  }

  async function prepareWritePlan(): Promise<void> {
    if (!project.value || !canBuildWritePlan.value) return;

    loading.value = true;
    error.value = undefined;
    try {
      if (isTauriRuntime()) {
        writePlan.value = await requestWritePlan({
          photoIds: selectedPhotoIds.value,
          outputDirectory: project.value.settings.outputDirectory ?? "",
          existingGpsPolicy: project.value.settings.existingGpsPolicy,
        });
      } else {
        writePlan.value = {
          id: "demo-write-plan",
          createdAt: new Date().toISOString(),
          outputDirectory: project.value.settings.outputDirectory ?? "",
          sourceFilesUnchanged: true,
          warnings:
            statistics.value.low > 0
              ? ["选择中包含低置信度照片，请逐张确认位置。"]
              : [],
          items: selectedPhotoIds.value.map((photoId) => {
            const photo = project.value?.photos.find(
              (item) => item.id === photoId,
            );
            const match = matchesByPhotoId.value.get(photoId);
            const writable = Boolean(
              match && MATCHED_STATUSES.includes(match.status),
            );

            return {
              photoId,
              sourcePath: photo?.path ?? "",
              outputPath: `${project.value?.settings.outputDirectory}/${photo?.relativePath}`,
              action: writable ? "WRITE" : "SKIP",
              reason: writable ? undefined : "没有可写入的匹配位置",
              oldGps: photo?.existingGps,
              newGps:
                writable && match?.lat !== undefined && match.lon !== undefined
                  ? {
                      lat: match.lat,
                      lon: match.lon,
                      altitude: match.elevation,
                    }
                  : undefined,
            };
          }),
        };
      }
      activeView.value = "write";
    } catch {
      error.value = {
        code: "WRITE_PERMISSION_DENIED",
        message: "无法生成写入计划。",
        suggestion: "确认输出目录与源照片目录不同，并检查输出目录权限。",
      };
    } finally {
      loading.value = false;
    }
  }

  function setView(view: AppView): void {
    if (!project.value && !["welcome", "settings"].includes(view)) return;
    activeView.value = view;
  }

  function dismissNotice(): void {
    notice.value = undefined;
  }

  function dismissError(): void {
    error.value = undefined;
  }

  return {
    project,
    activeView,
    activePhotoId,
    selectedPhotoIds,
    statusFilter,
    searchQuery,
    writePlan,
    tasks,
    loading,
    previewPending,
    notice,
    error,
    mapReady,
    mapBaseAvailable,
    matchesByPhotoId,
    activePhoto,
    activeMatch,
    filteredPhotos,
    statistics,
    selectedWritableCount,
    canBuildWritePlan,
    setProject,
    loadDemo,
    startNewProject,
    openExistingProject,
    persistProject,
    selectPhoto,
    togglePhotoSelection,
    selectFilteredPhotos,
    clearPhotoSelection,
    setFixedOffsetSeconds,
    prepareWritePlan,
    setView,
    dismissNotice,
    dismissError,
  };
});
