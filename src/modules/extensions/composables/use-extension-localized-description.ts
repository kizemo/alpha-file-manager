// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  ref,
  toValue,
  watch,
  type MaybeRefOrGetter,
  type Ref,
} from 'vue';
import { useI18n } from 'vue-i18n';
import {
  fetchUrlText,
  getExtensionRawFileUrl,
  getGitHubRepoInfo,
} from '@/data/extensions';
import { invokeAsExtension } from '@/modules/extensions/runtime/extension-invoke';
import type { ExtensionWithManifest } from '@/modules/extensions/composables/use-extensions';
import {
  extractExtensionDescription,
  getExtensionLocaleFileNames,
  isExtensionLocalizationNeeded,
  parseExtensionLocaleMessages,
} from '@/modules/extensions/utils/extension-locale';

/**
 * 远端最多尝试多少次请求。
 *
 * 一次渲染可能同时出现几十张卡片，而绝大多数第三方扩展并没有中文语言包，
 * 所以宁可少试几次、退回英文，也不要把市场列表变成一连串 404 请求。
 */
const MAX_REMOTE_ATTEMPTS = 3;

const descriptionCache = new Map<string, string | null>();
const inFlightRequests = new Map<string, Promise<string | null>>();

function resolveExtensionVersion(extension: ExtensionWithManifest): string | null {
  return extension.installedVersion
    || extension.latestVersion
    || extension.manifest?.version
    || null;
}

function buildRemoteRefs(extension: ExtensionWithManifest): string[] {
  const refs: string[] = [];
  const version = resolveExtensionVersion(extension);

  if (version) {
    refs.push(`v${version}`);
  }

  refs.push('main');

  return refs;
}

async function readLocalDescription(
  extension: ExtensionWithManifest,
  fileNames: string[],
): Promise<string | null> {
  for (const fileName of fileNames) {
    try {
      const fileBytes = await invokeAsExtension<number[]>(
        extension.id,
        'read_extension_file',
        {
          extensionId: extension.id,
          filePath: fileName,
        },
      );
      const text = new TextDecoder().decode(new Uint8Array(fileBytes));
      const description = extractExtensionDescription(parseExtensionLocaleMessages(text));

      if (description) {
        return description;
      }
    }
    catch {
      // 语言包不存在或读取失败：继续试下一个候选
    }
  }

  return null;
}

async function readRemoteDescription(
  extension: ExtensionWithManifest,
  fileNames: string[],
  refs: string[],
): Promise<string | null> {
  let attempts = 0;

  for (const fileName of fileNames) {
    for (const ref of refs) {
      if (attempts >= MAX_REMOTE_ATTEMPTS) {
        return null;
      }

      attempts += 1;

      try {
        const url = getExtensionRawFileUrl(extension.repository, ref, fileName);
        const result = await fetchUrlText(url);

        if (!result.ok) {
          continue;
        }

        const description = extractExtensionDescription(
          parseExtensionLocaleMessages(result.body),
        );

        if (description) {
          return description;
        }
      }
      catch {
        // 仓库地址非法或请求失败：继续试下一个候选
      }
    }
  }

  return null;
}

async function resolveDescription(
  extension: ExtensionWithManifest,
  appLocale: string,
): Promise<string | null> {
  if (!isExtensionLocalizationNeeded(appLocale)) {
    return null;
  }

  const fileNames = getExtensionLocaleFileNames(appLocale);

  if (fileNames.length === 0) {
    return null;
  }

  if (extension.isInstalled) {
    const localDescription = await readLocalDescription(extension, fileNames);

    if (localDescription) {
      return localDescription;
    }
  }

  if (getGitHubRepoInfo(extension.repository)) {
    return readRemoteDescription(extension, fileNames, buildRemoteRefs(extension));
  }

  return null;
}

function buildCacheKey(extension: ExtensionWithManifest, appLocale: string): string {
  return [
    extension.id,
    resolveExtensionVersion(extension) ?? '',
    extension.isInstalled ? 'installed' : 'remote',
    appLocale,
  ].join('::');
}

/**
 * 取扩展在当前界面语言下的说明文字；取不到返回 `null`，由调用方回退到市场英文描述。
 *
 * 结果按「扩展 id + 版本 + 是否已安装 + 语言」缓存，并做 in-flight 去重：
 * 同一张卡片在列表里被反复渲染也不会重复发请求。
 */
export async function loadExtensionLocalizedDescription(
  extension: ExtensionWithManifest,
  appLocale: string,
): Promise<string | null> {
  const cacheKey = buildCacheKey(extension, appLocale);

  if (descriptionCache.has(cacheKey)) {
    return descriptionCache.get(cacheKey) ?? null;
  }

  const existing = inFlightRequests.get(cacheKey);

  if (existing) {
    return existing;
  }

  const request = resolveDescription(extension, appLocale)
    .catch(() => null)
    .then((description) => {
      descriptionCache.set(cacheKey, description);
      inFlightRequests.delete(cacheKey);
      return description;
    });

  inFlightRequests.set(cacheKey, request);

  return request;
}

/** 供测试清空模块级缓存。 */
export function clearExtensionLocalizedDescriptionCache(): void {
  descriptionCache.clear();
  inFlightRequests.clear();
}

export function useExtensionLocalizedDescription(
  extension: MaybeRefOrGetter<ExtensionWithManifest>,
): { localizedDescription: Ref<string | null> } {
  const { locale } = useI18n();
  const localizedDescription = ref<string | null>(null);
  let generation = 0;

  watch(
    [() => toValue(extension), () => locale.value],
    async () => {
      const currentGeneration = ++generation;
      localizedDescription.value = null;
      const description = await loadExtensionLocalizedDescription(
        toValue(extension),
        locale.value,
      );

      if (currentGeneration === generation) {
        localizedDescription.value = description;
      }
    },
    { immediate: true },
  );

  return { localizedDescription };
}
