<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { PlusIcon, Trash2Icon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { toast } from '@/components/ui/toaster';
import { useExtensionsStore } from '@/stores/runtime/extensions';
import type { ExtensionMarketSource } from '@/types/extension';

const { t } = useI18n();
const extensionsStore = useExtensionsStore();

const isAddDialogOpen = ref(false);
const isRemoveDialogOpen = ref(false);
const pendingRemovalSourceId = ref<string | null>(null);
const sourceName = ref('');
const sourceUrl = ref('');
const isBusy = ref(false);

// 内置源（官方 + 第三方社区）由代码常量提供，不落盘、不可删、不可改地址；
// 它们排在自建源之前，顺序由 store 里的 allMarketSources 决定。
const builtInMarketSources = computed(() => {
  return extensionsStore.allMarketSources.filter(source => source.builtIn);
});

/** 来源类型标签。中文值由 i18n 提供，不在模板里拼字符串。 */
function sourceKindLabel(kind: ExtensionMarketSource['kind']): string {
  if (kind === 'official') {
    return t('extensions.marketSourceOfficial');
  }

  if (kind === 'community') {
    return t('extensions.marketSourceCommunity');
  }

  return t('extensions.marketSourceCustom');
}

const customMarketSources = computed(() => extensionsStore.marketSources);

const canSubmit = computed(() => {
  return sourceName.value.trim().length > 0 && sourceUrl.value.trim().length > 0;
});

function openAddDialog(): void {
  sourceName.value = '';
  sourceUrl.value = '';
  isAddDialogOpen.value = true;
}

async function handleAddSource(): Promise<void> {
  if (!canSubmit.value || isBusy.value) {
    return;
  }

  isBusy.value = true;

  try {
    const result = await extensionsStore.addMarketSource(sourceName.value, sourceUrl.value);

    if ('error' in result && result.error === 'invalid-url') {
      toast.error(t('extensions.marketSourceInvalidUrl'));
      return;
    }

    isAddDialogOpen.value = false;
  }
  catch (error) {
    toast.error(error instanceof Error ? error.message : t('extensions.marketSourceInvalidUrl'));
  }
  finally {
    isBusy.value = false;
  }
}

function sourceStatus(sourceId: string): string {
  return extensionsStore.registrySourceStates[sourceId]?.status ?? 'idle';
}

function openRemoveDialog(sourceId: string): void {
  pendingRemovalSourceId.value = sourceId;
  isRemoveDialogOpen.value = true;
}

async function handleRemoveSource(): Promise<void> {
  const sourceId = pendingRemovalSourceId.value;

  if (!sourceId || isBusy.value) {
    return;
  }

  isBusy.value = true;

  try {
    await extensionsStore.removeMarketSource(sourceId);
    isRemoveDialogOpen.value = false;
    pendingRemovalSourceId.value = null;
  }
  catch (error) {
    toast.error(error instanceof Error ? error.message : String(error));
  }
  finally {
    isBusy.value = false;
  }
}

async function handleToggleSource(sourceId: string, enabled: boolean): Promise<void> {
  try {
    await extensionsStore.setMarketSourceEnabled(sourceId, enabled);
  }
  catch (error) {
    toast.error(error instanceof Error ? error.message : String(error));
  }
}
</script>

<template>
  <Card class="market-sources">
    <CardHeader>
      <CardTitle class="market-sources__title">
        {{ t('extensions.marketSources') }}
      </CardTitle>
    </CardHeader>

    <CardContent class="market-sources__content">
      <div
        v-for="source in builtInMarketSources"
        :key="source.id"
        class="market-sources__row"
      >
        <div class="market-sources__info">
          <span class="market-sources__name">
            {{ source.name }}
          </span>
          <span class="market-sources__url">
            {{ source.url }}
          </span>
        </div>
        <div class="market-sources__meta">
          <span
            v-if="sourceStatus(source.id) === 'error'"
            class="market-sources__status market-sources__status--error"
          >
            {{ t('extensions.marketSourceUnreachable') }}
          </span>
          <span
            v-else-if="sourceStatus(source.id) === 'stale'"
            class="market-sources__status market-sources__status--stale"
          >
            {{ t('extensions.marketSourceUnreachable') }}
          </span>
          <span class="market-sources__kind">
            {{ sourceKindLabel(source.kind) }}
          </span>
        </div>
      </div>

      <p
        v-if="customMarketSources.length === 0"
        class="market-sources__empty"
      >
        {{ t('extensions.marketSourceEmpty') }}
      </p>

      <div
        v-for="source in customMarketSources"
        :key="source.id"
        class="market-sources__row"
      >
        <div class="market-sources__info">
          <span class="market-sources__name">
            {{ source.name }}
          </span>
          <span class="market-sources__url">
            {{ source.url }}
          </span>
        </div>
        <div class="market-sources__meta">
          <span
            v-if="sourceStatus(source.id) === 'error'"
            class="market-sources__status market-sources__status--error"
          >
            {{ t('extensions.marketSourceUnreachable') }}
          </span>
          <span
            v-else-if="sourceStatus(source.id) === 'stale'"
            class="market-sources__status market-sources__status--stale"
          >
            {{ t('extensions.marketSourceUnreachable') }}
          </span>
          <span class="market-sources__kind">
            {{ sourceKindLabel(source.kind) }}
          </span>
        </div>
        <div class="market-sources__actions">
          <Switch
            :model-value="source.enabled"
            :aria-label="source.name"
            @update:model-value="handleToggleSource(source.id, $event)"
          />
          <Button
            variant="ghost"
            size="icon"
            :aria-label="t('extensions.marketSourceRemove')"
            @click="openRemoveDialog(source.id)"
          >
            <Trash2Icon :size="16" />
          </Button>
        </div>
      </div>

      <Button
        variant="outline"
        class="market-sources__add"
        @click="openAddDialog"
      >
        <PlusIcon :size="16" />
        {{ t('extensions.marketSourceAdd') }}
      </Button>
    </CardContent>
  </Card>

  <Dialog v-model:open="isAddDialogOpen">
    <DialogContent>
      <DialogHeader>
        <DialogTitle>
          {{ t('extensions.marketSourceAdd') }}
        </DialogTitle>
        <DialogDescription>
          {{ t('extensions.marketSourceInvalidUrl') }}
        </DialogDescription>
      </DialogHeader>

      <div class="market-sources__form">
        <label class="market-sources__field">
          <span class="market-sources__field-label">
            {{ t('extensions.marketSourceName') }}
          </span>
          <Input
            v-model="sourceName"
            :placeholder="t('extensions.marketSourceName')"
          />
        </label>

        <label class="market-sources__field">
          <span class="market-sources__field-label">
            {{ t('extensions.marketSourceUrl') }}
          </span>
          <Input
            v-model="sourceUrl"
            placeholder="https://example.com/registry.json"
          />
        </label>
      </div>

      <DialogFooter>
        <Button
          variant="outline"
          @click="isAddDialogOpen = false"
        >
          {{ t('cancel') }}
        </Button>
        <Button
          :disabled="!canSubmit || isBusy"
          @click="handleAddSource"
        >
          {{ t('extensions.marketSourceAdd') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog v-model:open="isRemoveDialogOpen">
    <DialogContent>
      <DialogHeader>
        <DialogTitle>
          {{ t('extensions.marketSourceRemove') }}
        </DialogTitle>
        <DialogDescription>
          {{ t('extensions.marketSourceRemoveConfirm') }}
        </DialogDescription>
      </DialogHeader>

      <DialogFooter>
        <Button
          variant="outline"
          @click="isRemoveDialogOpen = false"
        >
          {{ t('cancel') }}
        </Button>
        <Button
          variant="destructive"
          :disabled="isBusy"
          @click="handleRemoveSource"
        >
          {{ t('extensions.marketSourceRemove') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
.market-sources__title {
  font-size: 1rem;
  font-weight: 600;
}

.market-sources__content {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.market-sources__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.5rem 0.75rem;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius-sm);
  gap: 0.75rem;
}

.market-sources__info {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.125rem;
}

.market-sources__name {
  overflow: hidden;
  color: hsl(var(--foreground));
  font-size: 0.875rem;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-sources__url {
  overflow: hidden;
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.market-sources__kind {
  flex-shrink: 0;
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
}

.market-sources__meta {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 0.5rem;
}

.market-sources__status {
  flex-shrink: 0;
  font-size: 0.75rem;
}

.market-sources__status--error {
  color: hsl(var(--destructive));
}

.market-sources__status--stale {
  color: hsl(var(--muted-foreground));
}

.market-sources__actions {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 0.5rem;
}

.market-sources__empty {
  color: hsl(var(--muted-foreground));
  font-size: 0.875rem;
}

.market-sources__add {
  align-self: flex-start;
}

.market-sources__form {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.market-sources__field {
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
}

.market-sources__field-label {
  color: hsl(var(--foreground));
  font-size: 0.875rem;
  font-weight: 500;
}
</style>
