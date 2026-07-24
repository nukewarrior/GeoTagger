import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

import { useWorkspaceStore } from "@/stores/workspace";

describe("workspace store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    delete window.__TAURI_INTERNALS__;
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("derives deterministic match statistics from the demo project", () => {
    const store = useWorkspaceStore();

    store.loadDemo();

    expect(store.activeView).toBe("workspace");
    expect(store.statistics.total).toBe(12);
    expect(store.statistics.high).toBeGreaterThan(0);
    expect(store.statistics.failed).toBe(3);
    expect(store.project?.schemaVersion).toBe(1);
    expect(store.project?.tracks[0].points.length).toBeGreaterThan(50);
  });

  it("keeps photo focus and batch selection as separate state", () => {
    const store = useWorkspaceStore();
    store.loadDemo();
    const photoId = store.project?.photos[1].id;
    expect(photoId).toBeDefined();
    if (!photoId) return;

    store.selectPhoto(photoId);
    expect(store.activePhotoId).toBe(photoId);

    const before = store.selectedPhotoIds.includes(photoId);
    store.togglePhotoSelection(photoId);
    expect(store.selectedPhotoIds.includes(photoId)).toBe(!before);
    expect(store.activePhotoId).toBe(photoId);
  });

  it("debounces fixed-offset preview recalculation", async () => {
    vi.useFakeTimers();
    const store = useWorkspaceStore();
    store.loadDemo();
    const originalConfidence = store.project?.matches[0].confidence ?? 0;

    store.setFixedOffsetSeconds(24);
    store.setFixedOffsetSeconds(26);

    expect(store.previewPending).toBe(true);
    await vi.runAllTimersAsync();
    expect(store.previewPending).toBe(false);
    expect(store.project?.settings.fixedOffsetMs).toBe(26_000);
    expect(store.project?.matches[0].confidence).toBeLessThan(originalConfidence);
  });

  it("builds a copy-only write plan for selected matched photos", async () => {
    const store = useWorkspaceStore();
    store.loadDemo();

    expect(store.canBuildWritePlan).toBe(true);
    await store.prepareWritePlan();

    expect(store.activeView).toBe("write");
    expect(store.writePlan?.sourceFilesUnchanged).toBe(true);
    expect(store.writePlan?.items.length).toBe(store.selectedPhotoIds.length);
    expect(
      store.writePlan?.items.every((item) =>
        item.outputPath.startsWith(
          store.project?.settings.outputDirectory ?? "missing",
        ),
      ),
    ).toBe(true);
  });
});
