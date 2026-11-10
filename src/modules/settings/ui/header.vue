<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { useI18n } from 'vue-i18n';
// FORK-MODIFICATION 2026-10-31（r14 需求 7）：设置页标题下的软件图标做「浅底版 / 深底版」两套配色。
//   浅底版（logo-1024x1024.png）    = 深色文件夹 + 浅色 α，用在浅色背景（浅色主题）。
//   深底版（logo-1024x1024-dark.png）= 浅米文件夹 + 深色 α，用在深色背景（暗色主题）。
// 切换开关是 <html> 上的 .dark class —— 与主题 token 走同一条链
// （见 src/stores/storage/composables/use-theme.ts 的 applyBaseTheme），
// 所以四套 AFM 主题（暖纸浅色 / 暖纸暗色 / 跃动活力 / 冷杉）自动各归其位，
// 不额外引入组件内状态，也不新增一条 watch。
import logoLightImage from '@/assets/icons/logo-1024x1024.png';
import logoDarkImage from '@/assets/icons/logo-1024x1024-dark.png';

const { t } = useI18n();
</script>

<template>
  <div class="settings-header">
    <!--
      FORK-MODIFICATION 2026-10-31（r14 需求 7）：两张图都渲染出来，用 display 互斥。
      只渲染一张再按主题改 src 的话，主题切换会触发一次图片重新加载，
      深色图标会闪一下白底。
    -->
    <img
      class="settings-header__logo settings-header__logo--light"
      :src="logoLightImage"
    >
    <img
      class="settings-header__logo settings-header__logo--dark"
      :src="logoDarkImage"
    >
    <div class="settings-header__title-section">
      <p class="settings-header__description">
        <!--
          FORK-MODIFICATION: 2026-10-13 换用用户给定的简介文案。

          原文案在 17 个语言包里都以 `"Alpha File Manager" ...` 开头,模板又在前面
          输出一次 app.name,于是渲染出两个连续的 `"Alpha File Manager"`,
          当初用 `.replace(/^"[^"]*"\s*/, '')` 把开头引语剥掉绕过。

          新文案本身已经是「加粗的品牌名 + 空格 + 正文」的结构:
            **Alpha File Manager** 基于 Sigma File Manager 二次开发,……
          所以 app.description 不再重复品牌名,引语也不需要了——
          正则与它对应的 FORK-MODIFICATION 注释一并删除,避免留下永不生效的兼容代码。
        -->
        <strong>{{ t('app.name') }}</strong>
        {{ t('app.description') }}
        <!--
          FORK-MODIFICATION: 验收反馈 2026-10-13 — 末尾增加官网与源码仓库链接。
          链接用 tauri-plugin-opener 的 open_external(若有),这里先用普通 <a target="_blank">。

          官网链接挂在「段燚」名字上,不再单独把 https://www.aiec.fun 这串地址显示出来;
          页面上只保留 GitHub 仓库地址作为可见链接文本。
          所以 app.developer 被拆成两个键:app.developerLabel(「开发:」/ "Developed by ")
          与 app.developer(人名本身)。

          分隔符用 &nbsp;&nbsp; 而不是两个半角空格:HTML 会把连续空白折叠成一个,
          模板里直接写两个空格渲染后只剩一个。也不能改用 white-space:pre —
          那会把模板自身的换行与缩进一起显示出来。

          注意 &nbsp;&nbsp; 必须与后面的 <a 紧挨着写。中间一旦换行,
          换行与缩进会被折叠成又一个空格,于是渲染出三个空格而不是两个
          (第一版就踩了这个坑,单测断言的是原始文本所以当场转红)。
        -->
        <br>
        {{ t('app.copyright') }}&nbsp;&nbsp;{{ t('app.developerLabel') }}<a
          class="settings-header__link"
          href="https://www.aiec.fun"
          target="_blank"
          rel="noopener noreferrer"
        >{{ t('app.developer') }}</a>&nbsp;&nbsp;<a
          class="settings-header__link"
          :href="t('app.repository')"
          target="_blank"
          rel="noopener noreferrer"
        >{{ t('app.repository') }}</a>
      </p>
    </div>
  </div>
</template>

<style scoped>
.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 1rem;
  border-bottom: 1px solid hsl(var(--border));
  gap: 2rem;
}

.settings-header img {
  width: 64px;
  height: 64px;
}

/* FORK-MODIFICATION 2026-10-31（r14 需求 7）：浅色主题显示浅底版图标，深色主题显示深底版。
   默认（无 .dark）= 浅底版；`.dark` 由 use-theme.ts 的 applyBaseTheme 写到 <html> 上，
   暖纸暗色主题的 baseTheme 就是 'dark'，所以同样落在这条分支里。 */

.settings-header__logo--dark {
  display: none;
}

:global(html.dark) .settings-header__logo--light {
  display: none;
}

:global(html.dark) .settings-header__logo--dark {
  display: block;
}

.settings-header__title-section {
  flex: 1;
}

.settings-header__description {
  margin: 0;
  color: hsl(var(--muted-foreground));
  font-size: calc(0.875rem + var(--font-size-offset, 0px));
}

.settings-header__link {
  color: hsl(var(--primary));
  text-decoration: none;
}

.settings-header__link:hover {
  text-decoration: underline;
}

@media (width <= 768px) {
  .settings-header {
    font-size: calc(10px + var(--font-size-offset, 0px));
    gap: 1rem;
  }

  .settings-header img {
    width: 36px;
    height: 36px;
  }

   .settings-header__description {
     font-size: calc(0.75rem + var(--font-size-offset, 0px));
   }
}
</style>
