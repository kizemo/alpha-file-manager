<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { SparklesIcon, SearchXIcon, Loader2Icon, WifiOffIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import ExtensionCard from './extension-card.vue';
import type { ExtensionWithManifest } from '@/modules/extensions/composables/use-extensions';
import type { ExtensionMarketSource } from '@/types/extension';
import type { RegistrySourceState } from '@/modules/extensions/utils/market-sources';
import { getStaggerSlideUpBinding } from '@/utils/stagger-animation';

const props = defineProps<{
  extensions: ExtensionWithManifest[];
  featuredExtensions: ExtensionWithManifest[];
  marketSources?: ExtensionMarketSource[];
  marketSourceStates?: Record<string, RegistrySourceState>;
  isLoading?: boolean;
  error?: string | null;
  installingExtensions: Set<string>;
  updatingExtensions: Set<string>;
  isAnyInstallInProgress?: boolean;
  searchQuery: string;
}>();

const emit = defineEmits<{
  select: [extension: ExtensionWithManifest, event: MouseEvent];
  install: [extensionId: string];
  update: [extensionId: string];
  cancel: [extensionId: string];
  refresh: [];
}>();

const { t } = useI18n();

const hasSearchQuery = computed(() => props.searchQuery.trim().length > 0);

const ALL_SOURCES_ID = 'all';

const activeSourceId = ref<string>(ALL_SOURCES_ID);

const sourceTabs = computed(() => {
  const tabs: Array<{
    id: string;
    name: string;
    status: string;
  }> = [
    {
      id: ALL_SOURCES_ID,
      name: t('extensions.marketSourceAll'),
      status: 'idle',
    },
  ];

  for (const source of props.marketSources ?? []) {
    tabs.push({
      id: source.id,
      name: source.name,
      status: props.marketSourceStates?.[source.id]?.status ?? 'idle',
    });
  }

  return tabs;
});

function matchesActiveSource(extension: ExtensionWithManifest): boolean {
  if (activeSourceId.value === ALL_SOURCES_ID) {
    return true;
  }

  return extension.sourceId === activeSourceId.value;
}

const sourceFilteredExtensions = computed(() => {
  return props.extensions.filter(matchesActiveSource);
});

const sourceFilteredFeaturedExtensions = computed(() => {
  return props.featuredExtensions.filter(matchesActiveSource);
});

const showFeatured = computed(() => {
  return !hasSearchQuery.value && sourceFilteredFeaturedExtensions.value.length > 0;
});

const displayExtensions = computed(() => {
  if (hasSearchQuery.value) {
    return sourceFilteredExtensions.value;
  }

  return sourceFilteredExtensions.value.filter(ext => !ext.featured);
});

const noResults = computed(() => {
  return hasSearchQuery.value && sourceFilteredExtensions.value.length === 0;
});

const showMarketplaceUnavailable = computed(() => {
  return Boolean(props.error)
    && sourceFilteredExtensions.value.length === 0
    && !hasSearchQuery.value;
});
</script>

<template>
  <div class="extensions-marketplace">
    <div
      v-if="isLoading"
      class="extensions-marketplace__loading"
    >
      <Loader2Icon
        :size="32"
        class="extensions-marketplace__loading-icon"
      />
      <p>{{ t('extensions.loadingRegistry') }}</p>
    </div>

    <div
      v-else-if="showMarketplaceUnavailable"
      class="extensions-marketplace__unavailable"
    >
      <EmptyState
        :icon="WifiOffIcon"
        :title="t('extensions.marketplaceUnavailable')"
        :description="t('extensions.marketplaceUnavailableDescription')"
      >
        <template #footer>
          <Button
            variant="outline"
            @click="emit('refresh')"
          >
            {{ t('extensions.refresh') }}
          </Button>
        </template>
      </EmptyState>
    </div>

    <template v-else>
      <div
        v-if="sourceTabs.length > 1"
        class="extensions-marketplace__sources"
      >
        <button
          v-for="tab in sourceTabs"
          :key="tab.id"
          type="button"
          :class="[
            'extensions-marketplace__source-tab',
            { 'extensions-marketplace__source-tab--active': tab.id === activeSourceId },
          ]"
          @click="activeSourceId = tab.id"
        >
          <span class="extensions-marketplace__source-tab-name">{{ tab.name }}</span>
          <span
            v-if="tab.status === 'error' || tab.status === 'stale'"
            class="extensions-marketplace__source-tab-status"
          >
            {{ t('extensions.marketSourceUnreachable') }}
          </span>
        </button>
      </div>

      <div
        v-if="noResults"
        class="extensions-marketplace__no-results"
      >
        <SearchXIcon
          :size="48"
          class="extensions-marketplace__no-results-icon"
        />
        <h3>{{ t('extensions.noResults') }}</h3>
        <p>{{ t('extensions.noResultsDescription', { query: searchQuery }) }}</p>
      </div>

      <template v-else>
        <div
          v-if="showFeatured"
          class="extensions-marketplace__section"
        >
          <div class="extensions-marketplace__section-header">
            <SparklesIcon :size="20" />
            <h2>{{ t('extensions.featured') }}</h2>
          </div>
          <div class="extensions-marketplace__grid extensions-marketplace__grid--featured">
            <ExtensionCard
              v-for="(extension, itemIndex) in sourceFilteredFeaturedExtensions"
              :key="extension.id"
              v-bind="getStaggerSlideUpBinding(itemIndex)"
              :extension="extension"
              :is-installing="installingExtensions.has(extension.id)"
              :is-updating="updatingExtensions.has(extension.id)"
              :is-install-disabled="installingExtensions.has(extension.id) || isAnyInstallInProgress"
              @click="(event) => emit('select', extension, event)"
              @install="emit('install', extension.id)"
              @update="emit('update', extension.id)"
              @cancel="emit('cancel', extension.id)"
            />
          </div>
        </div>

        <div
          v-if="displayExtensions.length > 0"
          class="extensions-marketplace__section"
        >
          <div class="extensions-marketplace__section-header">
            <h2>{{ hasSearchQuery ? t('extensions.searchResults') : t('extensions.otherExtensions') }}</h2>
            <span class="extensions-marketplace__count">{{ displayExtensions.length }}</span>
          </div>
          <div class="extensions-marketplace__grid">
            <ExtensionCard
              v-for="(extension, itemIndex) in displayExtensions"
              :key="extension.id"
              v-bind="getStaggerSlideUpBinding(itemIndex)"
              :extension="extension"
              :is-installing="installingExtensions.has(extension.id)"
              :is-updating="updatingExtensions.has(extension.id)"
              :is-install-disabled="installingExtensions.has(extension.id) || isAnyInstallInProgress"
              @click="(event) => emit('select', extension, event)"
              @install="emit('install', extension.id)"
              @update="emit('update', extension.id)"
              @cancel="emit('cancel', extension.id)"
            />
          </div>
        </div>

        <div
          v-if="sourceFilteredExtensions.length === 0 && !hasSearchQuery"
          class="extensions-marketplace__empty"
        >
          <p>{{ t('extensions.noExtensionsAvailable') }}</p>
        </div>
      </template>
    </template>
  </div>
</template>

<style>
.extensions-marketplace {
  display: flex;
  min-width: 0;
  max-width: 100%;
  flex-direction: column;
  gap: 32px;
}

.extensions-marketplace__sources {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.extensions-marketplace__source-tab {
  display: inline-flex;
  align-items: center;
  padding: 4px 12px;
  border: 1px solid hsl(var(--border));
  border-radius: 999px;
  background-color: transparent;
  color: hsl(var(--muted-foreground));
  cursor: pointer;
  font-size: 0.8125rem;
  gap: 6px;
}

.extensions-marketplace__source-tab:hover {
  color: hsl(var(--foreground));
}

.extensions-marketplace__source-tab--active {
  border-color: hsl(var(--primary));
  color: hsl(var(--foreground));
}

.extensions-marketplace__source-tab-name {
  overflow: hidden;
  max-width: 220px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.extensions-marketplace__source-tab-status {
  color: hsl(var(--muted-foreground));
  font-size: 0.6875rem;
}

.extensions-marketplace__loading,
.extensions-marketplace__unavailable {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 24px;
  color: hsl(var(--muted-foreground));
  gap: 12px;
  text-align: center;
}

.extensions-marketplace__loading-icon {
  animation: spin 1s linear infinite;
}

.extensions-marketplace__unavailable .empty-state {
  padding: 48px 24px;
}

.extensions-marketplace__no-results {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 24px;
  border-radius: var(--radius);
  background-color: hsl(var(--muted) / 30%);
  gap: 12px;
  text-align: center;
}

.extensions-marketplace__no-results-icon {
  color: hsl(var(--muted-foreground) / 50%);
}

.extensions-marketplace__no-results h3 {
  color: hsl(var(--foreground));
  font-size: 1.25rem;
  font-weight: 600;
}

.extensions-marketplace__no-results p {
  max-width: 400px;
  color: hsl(var(--muted-foreground));
  font-size: 0.875rem;
}

.extensions-marketplace__section {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.extensions-marketplace__section-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.extensions-marketplace__section-header h2 {
  color: hsl(var(--foreground));
  font-size: 1.25rem;
  font-weight: 600;
}

.extensions-marketplace__count {
  padding: 2px 8px;
  border-radius: 4px;
  background-color: hsl(var(--muted));
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
  font-weight: 500;
}

.extensions-marketplace__grid {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
}

.extensions-marketplace__grid--featured {
  grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
}

.extensions-marketplace__empty {
  padding: 24px;
  color: hsl(var(--muted-foreground));
  text-align: center;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }

  to {
    transform: rotate(360deg);
  }
}

@media (width <= 768px) {
  .extensions-marketplace__grid,
  .extensions-marketplace__grid--featured {
    grid-template-columns: 1fr;
  }
}
</style>
