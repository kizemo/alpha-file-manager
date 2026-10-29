import { fileURLToPath } from 'node:url';
import { mergeConfig, defineConfig, configDefaults } from 'vitest/config';
import viteConfig from './vite.config';

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      // `.superpowers/**` 与 `graphify-out/**` 是本机的一次性脚本区（前者整个
      // 目录不进版本库，后者是分析产物）。它们会在工作副本里留下成百上千个
      // `*.test.cjs` / `*.test.mjs`，vitest 默认的 `**/node_modules/**` 规则
      // 匹配不到它们（目录名被拍平成了 `node_modules__xxx`），于是这些
      // 外来用例被当成项目用例跑，还因为缺依赖而集体假红 —— 门禁必须量的是
      // 仓库里真正存在的那部分。与 eslint.config.ts 的 globalIgnores 保持一致。
      exclude: [...configDefaults.exclude, 'e2e/**', '.superpowers/**', 'graphify-out/**'],
      root: fileURLToPath(new URL('./', import.meta.url)),
      setupFiles: [fileURLToPath(new URL('./src/test-support/vitest-setup.ts', import.meta.url))],
      // FORK-MODIFICATION: 全量单测在本机会并行跑 175 个测试文件，
      // 默认 5 秒阈值下即便不改任何代码也会假红（基线同样复现）。
      // 假红比没有门禁更糟：它会诱导下一个人去"修"一个不存在的缺陷。
      testTimeout: 30000,
    },
  }),
);
