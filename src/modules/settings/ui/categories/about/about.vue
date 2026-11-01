<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  ExternalLinkIcon,
  FileTextIcon,
  GlobeIcon,
  QrCodeIcon,
  UserIcon,
} from '@lucide/vue';
import GithubIcon from '@/components/icons/github-icon.vue';
import { SettingsItem } from '@/modules/settings';
import { Button } from '@/components/ui/button';
import externalLinks from '@/data/external-links';
import appLogo from '@/assets/icons/logo-1024x1024-dark.png';
import wechatQrCode from '@/assets/images/wechat-qrcode.jpg';

const { t } = useI18n();

const currentVersion = ref('');

onMounted(async () => {
  try {
    currentVersion.value = await getVersion();
  }
  catch {
    currentVersion.value = '';
  }
});

function openExternal(url: string) {
  openUrl(url);
}
</script>

<template>
  <div class="about-section">
    <div class="about-section__hero">
      <img
        class="about-section__logo"
        :src="appLogo"
        :alt="t('app.name')"
      >
      <div class="about-section__hero-text">
        <div class="about-section__app-name">
          {{ t('app.name') }}
        </div>
        <div class="about-section__app-version">
          {{ `${t('currentVersion')}: ${currentVersion || '-'}` }}
        </div>
      </div>
      <Button
        variant="secondary"
        size="xs"
        @click="openExternal(externalLinks.githubChangelogLink)"
      >
        <FileTextIcon :size="16" />
        {{ t('settings.about.changelog') }}
      </Button>
    </div>

    <SettingsItem
      :title="t('settings.about.projectLinks')"
      :icon="GithubIcon"
    >
      <div class="about-section__links">
        <Button
          variant="secondary"
          size="xs"
          @click="openExternal(externalLinks.githubRepoLink)"
        >
          <ExternalLinkIcon :size="16" />
          {{ t('projectGithubButtons.projectPage') }}
        </Button>
        <Button
          variant="secondary"
          size="xs"
          @click="openExternal(externalLinks.githubCommitsLink)"
        >
          <ExternalLinkIcon :size="16" />
          {{ t('settings.about.commits') }}
        </Button>
        <Button
          variant="secondary"
          size="xs"
          @click="openExternal(externalLinks.githubIssuesLink)"
        >
          <ExternalLinkIcon :size="16" />
          {{ t('projectGithubButtons.requestsIssues') }}
        </Button>
      </div>
    </SettingsItem>

    <SettingsItem
      :title="t('settings.about.author')"
      :icon="UserIcon"
    >
      <div class="about-section__rows">
        <div class="about-section__row">
          <span class="about-section__row-label">
            {{ t('app.developerLabel') }}
          </span>
          <span class="about-section__row-value">
            {{ t('settings.about.developer') }}
          </span>
        </div>
        <div class="about-section__row">
          <span class="about-section__row-label">
            {{ t('app.copyright') }}
          </span>
        </div>
      </div>
    </SettingsItem>

    <SettingsItem
      :title="t('settings.about.website')"
      :icon="GlobeIcon"
    >
      <div>
        <Button
          variant="secondary"
          size="xs"
          @click="openExternal(externalLinks.learningNotesUrl)"
        >
          <ExternalLinkIcon :size="16" />
          {{ t('settings.actions.learningNotes') }}
        </Button>
      </div>
    </SettingsItem>

    <SettingsItem
      :title="t('settings.about.wechat')"
      :icon="QrCodeIcon"
    >
      <div class="about-section__wechat">
        <img
          class="about-section__qr"
          :src="wechatQrCode"
          :alt="t('settings.about.wechatAccount')"
        >
        <div class="about-section__wechat-text">
          <div class="about-section__wechat-name">
            {{ t('settings.about.wechatAccount') }}
          </div>
          <div class="about-section__wechat-hint">
            {{ t('settings.about.scanQrCode') }}
          </div>
        </div>
      </div>
    </SettingsItem>
  </div>
</template>

<style scoped>
.about-section {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.about-section__hero {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  padding: 1rem;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
  background-color: hsl(var(--surface-settings-card));
  gap: 1rem 1.5rem;
}

.about-section__logo {
  width: 64px;
  height: 64px;
  border-radius: 12px;
  object-fit: contain;
}

.about-section__hero-text {
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  gap: 0.25rem;
}

.about-section__app-name {
  color: hsl(var(--foreground));
  font-size: calc(1.125rem + var(--font-size-offset, 0px));
  font-weight: 600;
}

.about-section__app-version {
  color: hsl(var(--muted-foreground));
  font-family: monospace;
  font-size: calc(0.875rem + var(--font-size-offset, 0px));
}

.about-section__links {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.about-section__rows {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.about-section__row {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0.5rem;
}

.about-section__row-label {
  color: hsl(var(--muted-foreground));
  font-size: calc(0.875rem + var(--font-size-offset, 0px));
}

.about-section__row-value {
  color: hsl(var(--foreground));
  font-size: calc(0.875rem + var(--font-size-offset, 0px));
  font-weight: 600;
}

.about-section__wechat {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 1rem 1.5rem;
}

.about-section__qr {
  width: 144px;
  height: 144px;
  border: 1px solid hsl(var(--border));
  border-radius: 8px;
  background-color: #ffffff;
  object-fit: contain;
}

.about-section__wechat-text {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.25rem;
}

.about-section__wechat-name {
  color: hsl(var(--foreground));
  font-size: calc(0.875rem + var(--font-size-offset, 0px));
  font-weight: 600;
}

.about-section__wechat-hint {
  color: hsl(var(--muted-foreground));
  font-size: calc(0.8125rem + var(--font-size-offset, 0px));
}
</style>
