<script setup lang="ts">
import InspectorPanel from "@/features/calibration/InspectorPanel.vue";
import MapCanvas from "@/features/map/MapCanvas.vue";
import PhotoRail from "@/features/photos/PhotoRail.vue";
import TimelinePanel from "@/features/timeline/TimelinePanel.vue";
</script>

<template>
  <main class="workspace-view">
    <PhotoRail />
    <section class="workspace-center">
      <MapCanvas />
      <TimelinePanel />
    </section>
    <InspectorPanel />
  </main>
</template>
