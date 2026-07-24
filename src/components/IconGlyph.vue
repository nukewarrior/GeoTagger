<script setup lang="ts">
export type IconName =
  | "project"
  | "route"
  | "photo"
  | "clock"
  | "transform"
  | "preview"
  | "write"
  | "export"
  | "settings"
  | "help"
  | "map"
  | "layers"
  | "filter"
  | "check"
  | "warning"
  | "close"
  | "plus"
  | "minus"
  | "home"
  | "search"
  | "shield"
  | "play"
  | "save"
  | "tasks"
  | "chevron"
  | "folder";

withDefaults(
  defineProps<{
    name: IconName;
    size?: number;
    strokeWidth?: number;
  }>(),
  {
    size: 18,
    strokeWidth: 1.8,
  },
);
</script>

<template>
  <svg
    class="icon-glyph"
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    :stroke-width="strokeWidth"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <template v-if="name === 'project'">
      <path d="M3.5 6.5h6l2-2h9v15h-17z" />
      <path d="M3.5 8.5h17" />
    </template>
    <template v-else-if="name === 'route'">
      <circle cx="5" cy="18" r="2" />
      <circle cx="18.5" cy="5" r="2" />
      <path d="M6.7 16.9c3.2-2 1.7-5.5 4.8-7.2 1.8-1 3.2-.4 5-2.7" />
      <path d="m14.5 17 3.8 1.1-1.2-3.7" />
    </template>
    <template v-else-if="name === 'photo'">
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <circle cx="8.5" cy="9" r="1.5" />
      <path d="m4.5 17 4.8-4.8 3.2 3.1 2.3-2.1 4.7 3.8" />
    </template>
    <template v-else-if="name === 'clock'">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7v5l3.2 2" />
      <path d="M8 2.8 6.7 1.5M16 2.8l1.3-1.3" />
    </template>
    <template v-else-if="name === 'transform'">
      <circle cx="5" cy="17.5" r="2" />
      <circle cx="12" cy="6" r="2" />
      <circle cx="19" cy="17.5" r="2" />
      <path d="M6.3 15.9 10.8 7.7M13.2 7.7l4.5 8.2" />
      <path d="M8 18h8" />
    </template>
    <template v-else-if="name === 'preview'">
      <path d="M2.7 12s3.4-6 9.3-6 9.3 6 9.3 6-3.4 6-9.3 6-9.3-6-9.3-6Z" />
      <circle cx="12" cy="12" r="2.6" />
      <path d="m18.5 4.3.5-2m1.2 3.5 1.7-1.1" />
    </template>
    <template v-else-if="name === 'write'">
      <path d="M12 3v12" />
      <path d="m8.5 11.5 3.5 3.6 3.5-3.6" />
      <path d="M5 14.5v5h14v-5" />
    </template>
    <template v-else-if="name === 'export'">
      <path d="M14 3h-9v18h14V8z" />
      <path d="M14 3v5h5M9 14h7m-3-3 3 3-3 3" />
    </template>
    <template v-else-if="name === 'settings'">
      <circle cx="12" cy="12" r="2.8" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3V2.8h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z" />
    </template>
    <template v-else-if="name === 'help'">
      <circle cx="12" cy="12" r="9" />
      <path d="M9.8 9a2.4 2.4 0 1 1 3.3 2.2c-.8.4-1.1.9-1.1 1.8" />
      <path d="M12 17h.01" />
    </template>
    <template v-else-if="name === 'map'">
      <path d="m3 5 5-2 8 2 5-2v16l-5 2-8-2-5 2z" />
      <path d="M8 3v16M16 5v16" />
    </template>
    <template v-else-if="name === 'layers'">
      <path d="m12 3 9 5-9 5-9-5z" />
      <path d="m3 12 9 5 9-5M3 16l9 5 9-5" />
    </template>
    <template v-else-if="name === 'filter'">
      <path d="M3 5h18l-7 8v5l-4 2v-7z" />
    </template>
    <template v-else-if="name === 'check'">
      <circle cx="12" cy="12" r="9" />
      <path d="m8 12 2.6 2.6L16.5 9" />
    </template>
    <template v-else-if="name === 'warning'">
      <path d="M12 3 2.8 20h18.4z" />
      <path d="M12 9v4.5M12 17h.01" />
    </template>
    <template v-else-if="name === 'close'">
      <path d="m6 6 12 12M18 6 6 18" />
    </template>
    <template v-else-if="name === 'plus'">
      <path d="M12 5v14M5 12h14" />
    </template>
    <template v-else-if="name === 'minus'">
      <path d="M5 12h14" />
    </template>
    <template v-else-if="name === 'home'">
      <path d="m3 11 9-8 9 8" />
      <path d="M5.5 10v10h13V10M10 20v-6h4v6" />
    </template>
    <template v-else-if="name === 'search'">
      <circle cx="10.5" cy="10.5" r="6.5" />
      <path d="m15.5 15.5 5 5" />
    </template>
    <template v-else-if="name === 'shield'">
      <path d="M12 2.8 20 6v5.4c0 4.7-3.2 8.3-8 9.8-4.8-1.5-8-5.1-8-9.8V6z" />
      <path d="m8.7 12 2.1 2.1 4.7-4.7" />
    </template>
    <template v-else-if="name === 'play'">
      <circle cx="12" cy="12" r="9" />
      <path d="m10 8 6 4-6 4z" />
    </template>
    <template v-else-if="name === 'save'">
      <path d="M4 3h13l3 3v15H4z" />
      <path d="M8 3v6h8V3M8 21v-7h8v7" />
    </template>
    <template v-else-if="name === 'tasks'">
      <rect x="5" y="4" width="15" height="17" rx="2" />
      <path d="M9 4V2h7v2M9 9h7M9 13h7M9 17h4" />
      <path d="m2.5 9 1 1 2-2M2.5 14l1 1 2-2" />
    </template>
    <template v-else-if="name === 'chevron'">
      <path d="m8 10 4 4 4-4" />
    </template>
    <template v-else-if="name === 'folder'">
      <path d="M3 6h6l2-2h10v16H3z" />
    </template>
  </svg>
</template>
