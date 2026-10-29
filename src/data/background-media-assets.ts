// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-28：内置背景改为随安装包分发。
//
// 背景：上游 c0a16f8e「UX: reduce app size」把全尺寸内置背景踢出安装包，
// 改由前端在运行时去 raw.githubusercontent.com 拉原图。源站在大陆网络下不可达，
// 而下载器（src-tauri/src/background_sources.rs）既没有 connect timeout 也没有
// 总超时，于是「点默认背景 → 封面不刷新」= 请求永远挂着 + 异常被静默吞掉。
//
// 现在：全尺寸素材在构建期被 Vite 静态引入，随安装包一起分发。
// 缩略图仍然单独打包（source-backgrounds-previews），只用于背景管理器的网格预览。
// 运行期零网络 ⇒ 切换封面是同步的，不存在「等下载」的中间态。

const fullSizeBackgroundAssetModules = import.meta.glob('@/assets/media/source-backgrounds/*', {
  eager: true,
  import: 'default',
}) as Record<string, string>;

const previewBackgroundAssetModules = import.meta.glob('@/assets/media/source-backgrounds-previews/*.jpg', {
  eager: true,
  import: 'default',
}) as Record<string, string>;

export type BackgroundAssetIndex = Record<string, string>;

/**
 * 把 `import.meta.glob` 的产物（模块路径 → 资源 URL）折成「文件名 → 资源 URL」。
 * 键里可能带查询串或 Windows 反斜杠，所以按两种分隔符切最后一段。
 */
export function buildBackgroundAssetIndex(modules: Record<string, string>): BackgroundAssetIndex {
  const index: BackgroundAssetIndex = {};

  for (const [modulePath, url] of Object.entries(modules)) {
    const fileName = modulePath.split(/[\\/]/).pop() ?? '';

    if (fileName) {
      index[fileName] = url;
    }
  }

  return index;
}

/**
 * 预览图统一是 .jpg，而 3 个视频背景的原名是 .mp4 ⇒ 预览索引按「去扩展名的基名」建，
 * 否则视频在背景管理器网格里会取不到缩略图（老代码是按子串匹配的，等价效果）。
 */
export function buildBackgroundPreviewIndex(modules: Record<string, string>): BackgroundAssetIndex {
  const index: BackgroundAssetIndex = {};

  for (const [modulePath, url] of Object.entries(modules)) {
    const fileName = modulePath.split(/[\\/]/).pop() ?? '';

    if (fileName) {
      index[fileName.replace(/\.[^.]+$/, '')] = url;
    }
  }

  return index;
}

export const fullSizeBackgroundAssets = buildBackgroundAssetIndex(fullSizeBackgroundAssetModules);
export const previewBackgroundAssets = buildBackgroundPreviewIndex(previewBackgroundAssetModules);

export function resolveBackgroundAssetUrl(
  index: BackgroundAssetIndex,
  fileName: string,
): string | null {
  return index[fileName] ?? null;
}
