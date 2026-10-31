<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import {
  ref, onBeforeUnmount, onMounted, computed, watch, nextTick,
} from 'vue';
import { useI18n } from 'vue-i18n';
import { Tab, TabDraggable } from '.';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from '@/components/ui/tooltip';
import { ContextMenuShortcut } from '@/components/ui/context-menu';
import { useWorkspacesStore } from '@/stores/storage/workspaces';
import { useShortcutsStore } from '@/stores/runtime/shortcuts';
import { useHorizontalScrollFade } from '@/composables/use-horizontal-scroll-fade';
import { getScrollLeftToRevealInFadedContainer } from '@/composables/get-scroll-left-to-reveal-in-faded-container';
import type { Tab as TabType, TabGroup } from '@/types/workspaces';
import { PlusIcon } from '@lucide/vue';

const { t } = useI18n();

const props = withDefaults(defineProps<{
  teleportTarget?: string;
  compact?: boolean;
}>(), {
  teleportTarget: '.window-toolbar-primary-teleport-target',
  compact: false,
});

const workspacesStore = useWorkspacesStore();
const shortcutsStore = useShortcutsStore();

const tabGroupCount = computed(() => workspacesStore.currentWorkspace?.tabGroups?.length ?? 0);
// FORK-MODIFICATION 2026-10-30（意见 1）：标签组过多时按数量收窄「一个标签页」的宽度基准，
// 让最右侧的标签组尽量完整可见（配合左右滚动按钮兜底）。
const tabGroupBaseWidth = computed(() => {
  const count = tabGroupCount.value;

  if (count > 10) return '76px';
  if (count > 6) return '88px';

  return '100px';
});
const teleportDisabled = computed(() => !props.teleportTarget);
const teleportTo = computed(() => props.teleportTarget || 'body');
const { openNewTabGroup, closeTabGroup, setTabs } = workspacesStore;

const previewEnabled = ref(true);
const scrollContainerRef = ref<HTMLElement | null>(null);
const scrollContentRef = ref<HTMLElement | null>(null);
// FORK-MODIFICATION 2026-10-30（意见 1）：左右滚动按钮的可用状态。
const canScrollLeft = ref(false);
const canScrollRight = ref(false);
const TAB_SCROLL_STEP = 160;
let scrollDisableTimeoutId: number | null = null;
let scrollResizeObserver: ResizeObserver | null = null;
const { fadeWidth, scrollFadeClass, scrollFadeStyle, updateScrollFade } = useHorizontalScrollFade(scrollContainerRef, {
  scrollContentRef,
});

watch(tabGroupCount, (newCount, previousCount) => {
  nextTick(() => {
    if (previousCount !== undefined && newCount > previousCount) {
      scrollTabBarToEnd();
      return;
    }

    updateScrollFade();
    updateScrollButtons();
  });
});

watch(
  () => workspacesStore.currentTabGroup?.[0]?.id,
  (currentTabId, previousTabId) => {
    if (!currentTabId || currentTabId === previousTabId) {
      return;
    }

    scrollSelectedTabGroupIntoView();
  },
);

function handleScrollActivity() {
  previewEnabled.value = false;

  if (scrollDisableTimeoutId !== null) {
    clearTimeout(scrollDisableTimeoutId);
  }

  scrollDisableTimeoutId = window.setTimeout(() => {
    previewEnabled.value = true;
  }, 200);
}

function handleWheel(event: WheelEvent) {
  const container = scrollContainerRef.value;
  if (!container) return;
  container.scrollLeft += event.deltaY || event.deltaX || 0;
  handleScrollActivity();
}

function onScroll() {
  handleScrollActivity();
  updateScrollFade();
  updateScrollButtons();
}

// FORK-MODIFICATION 2026-10-30（意见 1）：按容器实际可滚动距离同步左右按钮的显隐。
function updateScrollButtons() {
  const container = scrollContainerRef.value;

  if (!container) {
    canScrollLeft.value = false;
    canScrollRight.value = false;

    return;
  }

  // 标签条本身可能比容器宽（真正的裁切发生在祖先元素上），
  // 所以要用「内容宽度」和「容器可视宽度」比较，而不是 container.scrollWidth。
  const content = scrollContentRef.value;
  const contentWidth = content
    ? Math.max(content.scrollWidth, content.offsetWidth)
    : container.scrollWidth;
  const maxScrollLeft = Math.max(contentWidth - container.clientWidth, 0);

  canScrollLeft.value = container.scrollLeft > 1;
  canScrollRight.value = maxScrollLeft - container.scrollLeft > 1;
}

function scrollTabsBy(direction: -1 | 1) {
  const container = scrollContainerRef.value;

  if (!container) return;

  container.scrollTo({
    left: container.scrollLeft + direction * TAB_SCROLL_STEP,
    behavior: 'smooth',
  });

  updateScrollFade();
}

function handleWindowResize() {
  updateScrollFade();
  updateScrollButtons();
}

function scrollTabBarToEnd() {
  nextTick(() => {
    const container = scrollContainerRef.value;
    if (!container) return;

    container.scrollTo({
      left: container.scrollWidth - container.clientWidth,
      behavior: 'smooth',
    });

    updateScrollFade();
    updateScrollButtons();
  });
}

function scrollSelectedTabGroupIntoView() {
  nextTick(() => {
    const container = scrollContainerRef.value;
    const selectedTab = container?.querySelector('.tab[is-active="true"]');

    if (!container || !(selectedTab instanceof HTMLElement)) {
      return;
    }

    const elementOffsetLeft = selectedTab.getBoundingClientRect().left
      - container.getBoundingClientRect().left
      + container.scrollLeft;

    container.scrollTo({
      left: getScrollLeftToRevealInFadedContainer({
        containerScrollLeft: container.scrollLeft,
        containerClientWidth: container.clientWidth,
        containerScrollWidth: container.scrollWidth,
        elementOffsetLeft,
        elementWidth: selectedTab.offsetWidth,
        fadeWidth,
      }),
      behavior: 'instant',
    });

    updateScrollFade();
    updateScrollButtons();
  });
}

function scheduleScrollButtonUpdate() {
  window.requestAnimationFrame(() => {
    updateScrollFade();
    updateScrollButtons();
  });
}

onMounted(() => {
  scrollSelectedTabGroupIntoView();
  updateScrollButtons();
  scheduleScrollButtonUpdate();
  window.addEventListener('resize', handleWindowResize);

  // 标签条的真实宽度只有布局稳定后才知道（工具栏伸缩、字体加载、标签组数量变化）。
  // 只在挂载时量一次会得到「还没有溢出」的旧值，左右滚动按钮就永远不出现。
  if (typeof ResizeObserver !== 'undefined') {
    scrollResizeObserver = new ResizeObserver(() => {
      updateScrollFade();
      updateScrollButtons();
    });

    if (scrollContainerRef.value) {
      scrollResizeObserver.observe(scrollContainerRef.value);
    }

    if (scrollContentRef.value) {
      scrollResizeObserver.observe(scrollContentRef.value);
    }
  }
});

onBeforeUnmount(() => {
  if (scrollDisableTimeoutId !== null) {
    clearTimeout(scrollDisableTimeoutId);
  }

  if (scrollResizeObserver) {
    scrollResizeObserver.disconnect();
    scrollResizeObserver = null;
  }

  window.removeEventListener('resize', handleWindowResize);
});
</script>

<template>
  <Teleport
    :to="teleportTo"
    :disabled="teleportDisabled"
  >
    <div
      class="tab-bar animate-sigma-ui-fade-in"
      :class="{ 'tab-bar--compact': compact }"
    >
      <div class="tab-bar__scroll-wrapper">
        <button
          type="button"
          class="tab-bar__scroll-button tab-bar__scroll-button--left"
          :class="{ 'tab-bar__scroll-button--hidden': !canScrollLeft }"
          :aria-label="t('tabs.scrollTabsLeft')"
          :title="t('tabs.scrollTabsLeft')"
          @click="scrollTabsBy(-1)"
        />
        <div
          ref="scrollContainerRef"
          class="tab-bar__base-container"
          :class="scrollFadeClass"
          :style="scrollFadeStyle"
          @wheel.prevent="handleWheel"
          @scroll="onScroll"
        >
          <div
            ref="scrollContentRef"
            class="tab-bar__base"
            :style="{ '--tab-group-base-width': tabGroupBaseWidth }"
          >
            <TabDraggable
              :items="workspacesStore.currentWorkspace?.tabGroups || []"
              :draggable-bg-color-var="'window-toolbar-color'"
              parent-selector=".tab-bar"
              @set="setTabs($event as TabGroup[])"
              @drag-start="previewEnabled = false"
              @drag-end="previewEnabled = true"
            >
              <template #item="{ item }">
                <Tab
                  :tab-group="(item as TabType[])"
                  :preview-enabled="previewEnabled"
                  @close-tab="closeTabGroup($event)"
                />
              </template>
            </TabDraggable>
          </div>
        </div>
        <button
          type="button"
          class="tab-bar__scroll-button tab-bar__scroll-button--right"
          :class="{ 'tab-bar__scroll-button--hidden': !canScrollRight }"
          :aria-label="t('tabs.scrollTabsRight')"
          :title="t('tabs.scrollTabsRight')"
          @click="scrollTabsBy(1)"
        />
      </div>

      <Tooltip>
        <TooltipTrigger as-child>
          <Button
            class="tab-bar__add-tab-button"
            variant="ghost"
            size="xs"
            @click="openNewTabGroup()"
          >
            <PlusIcon :size="14" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          <div class="tab-bar__tooltip">
            <div class="tab-bar__tooltip-row">
              {{ t('tabs.newTab') }}
              <ContextMenuShortcut>{{ shortcutsStore.getShortcutLabel('openNewTab') }}</ContextMenuShortcut>
            </div>
            <div class="tab-bar__tooltip-row">
              {{ t('shortcuts.duplicateCurrentTab') }}
              <ContextMenuShortcut>{{ shortcutsStore.getShortcutLabel('duplicateCurrentTab') }}</ContextMenuShortcut>
            </div>
          </div>
        </TooltipContent>
      </Tooltip>
    </div>
  </Teleport>
</template>

<style>
.tab-bar,
.tab-bar * {
  -webkit-app-region: no-drag;
}

.tab-bar {
  display: flex;
  max-width: 100%;
  height: 100%;
  align-items: center;
  gap: 4px;
}

.tab-bar__base-container {
  display: flex;
  overflow: auto;
  min-width: 0;
  max-width: 100%;
  flex: 1 1 0;
  align-items: center;
  scrollbar-width: none;
}

/* FORK-MODIFICATION 2026-10-30（意见 1）：标签组滚动条容器 + 左右滚动按钮。 */
.tab-bar__scroll-wrapper {
  position: relative;
  display: flex;
  overflow: hidden;
  min-width: 0;
  flex: 1 1 auto;
  align-items: center;
}

.tab-bar__scroll-button {
  position: absolute;
  z-index: 2;
  top: 50%;
  display: flex;
  width: 24px;
  height: var(--tab-height);
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--radius-sm);
  background-color: hsl(var(--card) / 92%);
  color: hsl(var(--foreground));
  cursor: pointer;
  transform: translateY(-50%);
}

.tab-bar__scroll-button::before {
  display: block;
  width: 6px;
  height: 6px;
  border-top: 1.5px solid currentcolor;
  border-right: 1.5px solid currentcolor;
  content: "";
}

.tab-bar__scroll-button--left {
  left: 2px;
}

.tab-bar__scroll-button--left::before {
  transform: rotate(-135deg);
}

.tab-bar__scroll-button--right {
  right: 2px;
}

.tab-bar__scroll-button--right::before {
  transform: rotate(45deg);
}

.tab-bar__scroll-button--hidden {
  visibility: hidden;
}

.tab-bar__base {
  display: flex;
  width: max-content;
  height: calc(var(--window-toolbar-height) - 4px);
  align-items: center;
  justify-content: center;
}

.tab-bar__add-tab-button {
  color: hsl(var(--muted-foreground));
}

.tab-bar__tooltip {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.tab-bar__tooltip-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.tab-bar--compact {
  width: 100%;
  height: var(--window-toolbar-height);
  padding: 0;
}

.tab-bar--compact .tab-bar__base-container {
  overflow: auto hidden;
  min-width: 0;
  flex: 1;
}
</style>
