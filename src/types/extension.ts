// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type {
  BinaryInfo,
  BinaryPathPreference,
  ContextMenuContext,
  ExtensionActivationContext,
  ExtensionActivationEvent,
  ExtensionCommand,
  ExtensionContextMenuItem,
  ExtensionKeybindingWhen,
  ExtensionManifest,
  ExtensionPublisher,
  ExtensionScopedDirectory,
  ExtensionSidebarItem,
  ExtensionToolbarDropdown,
} from '@sigma-file-manager/api';

export type * from '@sigma-file-manager/api';

export type ExtensionRegistryEntry = {
  id: string;
  name: string;
  description: string;
  publisher: ExtensionPublisher;
  repository: string;
  featured: boolean;
  categories: string[];
  tags?: string[];
  integrity?: string;
  releaseMetadata?: Record<string, {
    integrity?: string;
    manifest?: string;
  }>;
};

export type ExtensionRegistry = {
  schemaVersion: string;
  extensions: ExtensionRegistryEntry[];
};

/**
 * 市场源类型。
 *
 * - `official`：官方市场源（内置常量，不落盘、不可删、不可改地址）。
 * - `custom`：用户自建市场源（落盘、可增删、可停用）。
 */
// `official` / `community` 都是代码内置源（不落盘、不可删、不可改地址）：
//   - official  = 本项目自己的市场，排第一，市场页不显示来源标签
//   - community = 上游社区市场，排在 official 之后，条目上标来源
// `custom` 是用户自建源，可停用、可删除。
export type ExtensionMarketSourceKind = 'official' | 'community' | 'custom';

export type ExtensionMarketSource = {
  id: string;
  name: string;
  url: string;
  kind: ExtensionMarketSourceKind;
  enabled: boolean;
  builtIn?: boolean;
};

/** 条目带上来源，便于市场页按来源过滤、卡片标注来源。 */
export type ExtensionRegistryEntryWithSource = ExtensionRegistryEntry & {
  sourceId: string;
};

/** 多源合并后的清单：每条条目都带上来源 id。 */
export type MergedExtensionRegistry = {
  schemaVersion: string;
  extensions: ExtensionRegistryEntryWithSource[];
};

/** 按源存放的清单缓存（Phase 1 起取代单数的 `registryCache`）。 */
export type ExtensionRegistryCache = {
  data: ExtensionRegistry;
  fetchedAt: number;
};

export type ExtensionShortcutScope = 'local' | 'global';

export type ExtensionKeybindingOverride = {
  commandId: string;
  scope?: ExtensionShortcutScope;
  keys: {
    ctrl?: boolean;
    alt?: boolean;
    shift?: boolean;
    meta?: boolean;
    key: string;
  };
};

export type ExtensionSettings = {
  scopedDirectories: ExtensionScopedDirectory[];
  customSettings?: Record<string, unknown>;
  configurationValues?: Record<string, unknown>;
  keybindingOverrides?: ExtensionKeybindingOverride[];
};

export type InstalledExtension = {
  id: string;
  version: string;
  enabled: boolean;
  autoUpdate: boolean;
  installedAt: number;
  manifest: ExtensionManifest;
  settings: ExtensionSettings;
  registryEntry?: ExtensionRegistryEntry;
  isLocal?: boolean;
  localSourcePath?: string;
  installPendingDependencies?: boolean;
};

export type ExtensionLoadState = 'unloaded' | 'loading' | 'loaded' | 'error';

export type ExtensionActivateContext = ExtensionActivationContext;

export type ExtensionInstance = {
  activate: (context?: ExtensionActivateContext) => Promise<void> | void;
  deactivate: () => Promise<void> | void;
};

export type LoadedExtension = {
  id: string;
  state: ExtensionLoadState;
  activationEvent?: ExtensionActivationEvent;
  error?: string;
  instance?: ExtensionInstance;
};

export type InstalledExtensionData = {
  version: string;
  enabled: boolean;
  autoUpdate: boolean;
  installedAt: number;
  manifest: ExtensionManifest;
  settings: ExtensionSettings;
  isLocal?: boolean;
  localSourcePath?: string;
  installPendingDependencies?: boolean;
};

export type SharedBinaryInfo = BinaryInfo & {
  usedBy: string[];
};

export type ExtensionStorageData = {
  installedExtensions: Record<string, InstalledExtensionData>;
  sharedBinaries: Record<string, SharedBinaryInfo>;
  customBinaryPreferences: Record<string, BinaryPathPreference>;
  registryCache?: {
    data: ExtensionRegistry;
    fetchedAt: number;
  };
  /** 按市场源分别存放的清单缓存（Phase 1 起取代单数的 `registryCache`）。 */
  registryCaches?: Record<string, ExtensionRegistryCache>;
  /** 用户自建的市场源；官方源是内置常量，不落盘。 */
  marketSources?: ExtensionMarketSource[];
  recentCommandIds?: string[];
};

export type ContextMenuItemRegistration = {
  extensionId: string;
  item: ExtensionContextMenuItem;
  handler: (context: ContextMenuContext) => Promise<void> | void;
};

export type SidebarPageRegistration = {
  extensionId: string;
  page: ExtensionSidebarItem;
  component?: unknown;
  url?: string;
};

export type ToolbarDropdownRegistration = {
  extensionId: string;
  dropdown: ExtensionToolbarDropdown;
  handlers: Record<string, () => Promise<void> | void>;
};

export type CommandRegistration = {
  extensionId: string;
  command: ExtensionCommand;
  handler: (...args: unknown[]) => Promise<unknown> | unknown;
};

export type ExtensionKeybindingRegistration = {
  extensionId: string;
  commandId: string;
  keys: {
    ctrl?: boolean;
    alt?: boolean;
    shift?: boolean;
    meta?: boolean;
    key: string;
  };
  when?: ExtensionKeybindingWhen;
};

export type ModalSubmitCallback = (values: Record<string, unknown>, buttonId: string) => void | boolean | Promise<void | boolean>;
export type ModalCloseCallback = () => void;
export type ModalValueChangeCallback = (elementId: string, value: unknown, allValues: Record<string, unknown>) => void;
