import { globalIgnores } from 'eslint/config';
import { defineConfigWithVueTs, vueTsConfigs } from '@vue/eslint-config-typescript';
import pluginVue from 'eslint-plugin-vue';
import pluginVitest from '@vitest/eslint-plugin';
import pluginStylistic from '@stylistic/eslint-plugin';
import checkFile from 'eslint-plugin-check-file';
import pluginVueI18n from '@intlify/eslint-plugin-vue-i18n';

export default defineConfigWithVueTs(
  // `_acceptance/**` 是真机验收的临时探针区（NSIS 探针工程、一次性 .mjs/.ps1 脚本）。
  // 它与 `.gitignore` 里的 `/_acceptance/` 是同一件事，但 eslint 不读 .gitignore ——
  // 少了这条规则，光探针脚本就会往门禁里灌进 600 多个错误，把真实问题淹掉。
  //
  // `.superpowers/**` 同理：那是本机的一次性脚本区（`.superpowers/sdd/.gitignore`
  // 里是 `*`，整个目录不进版本库）。它只在工作副本里存在，干净检出里根本没有，
  // 所以留着它 lint 会让 `eslint .` 的数字**不可复现** —— 同一份代码，在工作副本里
  // 是 689 个错误，在干净检出里是 177 个。门禁必须量的是仓库里真正存在的那部分。
  // `extensions/**/src/generated-*.js` 是扩展构建脚本生成的 i18n 兜底表
  // （见 `extensions/kizemo.git/build.mjs`），跟 `dist/` 一样是产物而非手写源码，
  // 不该进仓库门禁。其余 `extensions/**/src/*.js` 是手写源码，必须过 lint。
  //
  // `scripts/archive-focus-sync/**` 是 2026-11-10 换根时从外层仓归档进来的
  // focus-sync 时代一次性探针（`scan-sandbox.cjs` 等），不再维护、也不属于产品代码。
  // 不忽略它会让门禁基线凭空多出 2 个错误（`no-require-imports`、`max-statements-per-line`）。
  globalIgnores(['**/dist/**', '**/dist-ssr/**', '**/coverage/**', '**/e2e-webdriver/**', 'src-tauri/**', '_acceptance/**', '.superpowers/**', 'graphify-out/**', 'extensions/**/src/generated-*.js', 'scripts/archive-focus-sync/**']),
  pluginVue.configs['flat/strongly-recommended'],
  vueTsConfigs.recommended,
  pluginStylistic.configs.customize({
    semi: true,
  }),
  {
    ...pluginVitest.configs.recommended,
    files: ['src/**/__tests__/*'],
    rules: {
      ...pluginVitest.configs.recommended.rules,
      'vitest/valid-expect': 'off',
    },
  },
  {
    // `scripts/*.cjs` 是 CommonJS 形态的门禁脚本（focus-sync 安装链的
    // `extensions/kizemo.focus-sync/installer/*.ps1` 硬编码了它们的 `.cjs` 路径，
    // 改扩展名或转 ESM 会破坏安装链且没有功能收益），故在此豁免 require 规则。
    files: ['**/*.cjs'],
    rules: {
      '@typescript-eslint/no-require-imports': 'off',
    },
  },
  {
    files: ['**/*.{ts,mts,tsx,vue}'],
    plugins: {
      'check-file': checkFile,
    },
    rules: {
      'func-style': ['error', 'declaration', { allowTypeAnnotation: true }],
      'vue/max-attributes-per-line': 'error',
      'vue/multi-word-component-names': 'off',
      'vue/one-component-per-file': 'off',
      'vue/component-name-in-template-casing': ['error', 'PascalCase', {
        registeredComponentsOnly: true,
        ignores: [],
        globals: ['RouterView', 'Teleport', 'Component', 'Transition', 'TransitionGroup'],
      }],
      '@stylistic/object-curly-spacing': ['error', 'always'],
      '@stylistic/object-property-newline': ['error', { allowAllPropertiesOnSameLine: false }],
      '@stylistic/object-curly-newline': [
        'error',
        {
          ObjectExpression: {
            multiline: true,
            consistent: true,
          },
          ObjectPattern: { multiline: true },
          ImportDeclaration: {
            multiline: true,
            minProperties: 5,
          },
          ExportDeclaration: { multiline: true },
          TSTypeLiteral: {
            multiline: true,
            consistent: true,
            minProperties: 2,
          },
          TSInterfaceBody: {
            multiline: true,
            consistent: true,
            minProperties: 2,
          },
        },
      ],
      '@stylistic/padding-line-between-statements': [
        'error',
        {
          blankLine: 'always',
          prev: 'block-like',
          next: '*',
        },
        {
          blankLine: 'always',
          prev: '*',
          next: 'block-like',
        },
      ],
      'check-file/filename-naming-convention': [
        'error',
        {
          '**/*': 'KEBAB_CASE',
        },
        {
          ignoreMiddleExtensions: true,
        },
      ],
      'check-file/folder-naming-convention': [
        'error',
        {
          '**/': 'KEBAB_CASE',
        },
        {
          ignoreWords: ['__tests__'],
        },
      ],
    },
    settings: {
      'vue-i18n': {
        localeDir: './src/localization/messages/*.json',
      },
    },
  },
  {
    files: ['src/**/*.vue'],
    plugins: {
      '@intlify/vue-i18n': pluginVueI18n,
    },
    rules: {
      '@intlify/vue-i18n/no-raw-text': [
        'error',
        {
          ignoreNodes: ['v-icon'],
          ignoreText: [
            '(',
            ')',
            '(+',
            '[',
            ']',
            '{',
            '}',
            ',',
            '.',
            '...',
            ';',
            ':',
            '"',
            '\'',
            '+',
            '-',
            '=',
            '%',
            '@',
            '|',
            '/',
            '\\',
            ' ',
            '—',
            '·',
            'Alt',
            'Ctrl',
            'Shift',
            'Meta',
            'Enter',
            '⚠',
            'Esc',
            'Tab',
            'Shift+Tab',
            'Ctrl+F',
            'Ctrl+Shift+G',
            'Ctrl+P',
            '↑↓',
          ],
        },
      ],
    },
  },
);
