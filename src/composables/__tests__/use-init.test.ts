// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  afterEach, beforeEach, describe, expect, it, vi,
} from 'vitest';

const {
  appWindowInitMainWindowStateListenersMock,
  applyLaunchAtStartupPreferenceMock,
  archiveJobsEnsureEventListenersMock,
  backgroundMediaRefreshCustomBackgroundsMock,
  checkAndShowChangelogMock,
  copyMoveJobsEnsureEventListenersMock,
  deleteJobsEnsureEventListenersMock,
  disableWebViewFeaturesMock,
  extensionsInitMock,
  getDirEntryMock,
  globalSearchInitOnLaunchMock,
  globalShortcutsInitMock,
  hideWindowMock,
  initAutoCheckMock,
  invokeMock,
  openOrFocusTabGroupMock,
  platformInitMock,
  quickViewEnsureMainWindowDisplayedPathListenerMock,
  removeAppSplashMock,
  remoteConnectionsInitMock,
  resolveLaunchTargetsFromArgsMock,
  routerPushMock,
  setFocusMock,
  setPendingLaunchRevealMock,
  shortcutsInitMock,
  showWindowMock,
  terminalsInitMock,
  userPathsInitMock,
  userSettingsInitMock,
  userStatsInitMock,
  userStatsRunDeferredMaintenanceMock,
  workspacesInitMock,
  workspacesLoadCurrentTabGroupMock,
  userSettingsMock,
  userSettingsSetMock,
} = vi.hoisted(() => ({
  appWindowInitMainWindowStateListenersMock: vi.fn(),
  applyLaunchAtStartupPreferenceMock: vi.fn(),
  archiveJobsEnsureEventListenersMock: vi.fn(),
  backgroundMediaRefreshCustomBackgroundsMock: vi.fn(),
  checkAndShowChangelogMock: vi.fn(),
  copyMoveJobsEnsureEventListenersMock: vi.fn(),
  deleteJobsEnsureEventListenersMock: vi.fn(),
  disableWebViewFeaturesMock: vi.fn(),
  extensionsInitMock: vi.fn(),
  getDirEntryMock: vi.fn(),
  globalSearchInitOnLaunchMock: vi.fn(),
  globalShortcutsInitMock: vi.fn(),
  hideWindowMock: vi.fn(),
  initAutoCheckMock: vi.fn(),
  invokeMock: vi.fn(),
  openOrFocusTabGroupMock: vi.fn(),
  platformInitMock: vi.fn(),
  quickViewEnsureMainWindowDisplayedPathListenerMock: vi.fn(),
  removeAppSplashMock: vi.fn(),
  remoteConnectionsInitMock: vi.fn(),
  resolveLaunchTargetsFromArgsMock: vi.fn(),
  routerPushMock: vi.fn(),
  setFocusMock: vi.fn(),
  setPendingLaunchRevealMock: vi.fn(),
  shortcutsInitMock: vi.fn(),
  showWindowMock: vi.fn(),
  terminalsInitMock: vi.fn(),
  userPathsInitMock: vi.fn(),
  userSettingsInitMock: vi.fn(),
  userStatsInitMock: vi.fn(),
  userStatsRunDeferredMaintenanceMock: vi.fn(),
  workspacesInitMock: vi.fn(),
  workspacesLoadCurrentTabGroupMock: vi.fn(),
  userSettingsMock: {
    launchAtStartup: false,
    launchAtStartupHidden: false,
    startupPage: 'home' as 'last' | 'home' | 'dashboard' | 'navigator',
    lastRoute: {
      name: 'home' as 'home' | 'navigator' | 'dashboard' | 'settings' | 'extensions' | 'extension-page',
      fullPageId: '',
    },
  },
  userSettingsSetMock: vi.fn(),
}));

// FORK-MODIFICATION 2026-11-19（批次 23）：设备打开失败时的可见提示。
const { openInSystemExplorerMock, toastCustomMock } = vi.hoisted(() => ({
  openInSystemExplorerMock: vi.fn(),
  toastCustomMock: vi.fn(),
}));

vi.mock('vue', async () => {
  const actual = await vi.importActual<typeof import('vue')>('vue');
  return {
    ...actual,
    nextTick: () => Promise.resolve(),
    onMounted: () => {},
    onUnmounted: () => {},
  };
});

vi.mock('vue-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue-router')>();
  return {
    ...actual,
    useRouter: () => ({
      push: routerPushMock,
      afterEach: vi.fn(),
      currentRoute: {
        value: {
          name: 'home',
          params: {},
        },
      },
    }),
  };
});

vi.mock('@/router', () => ({
  default: {
    push: routerPushMock,
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    label: 'main',
  }),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    label: 'main',
    show: showWindowMock,
    hide: hideWindowMock,
    setFocus: setFocusMock,
  }),
}));

vi.mock('@/stores/storage/user-paths', () => ({
  useUserPathsStore: () => ({
    init: userPathsInitMock,
  }),
}));

vi.mock('@/stores/storage/user-settings', () => ({
  useUserSettingsStore: () => ({
    init: userSettingsInitMock,
    userSettings: userSettingsMock,
    set: userSettingsSetMock,
  }),
}));

vi.mock('@/stores/storage/user-stats', () => ({
  useUserStatsStore: () => ({
    init: userStatsInitMock,
    runDeferredMaintenance: userStatsRunDeferredMaintenanceMock,
  }),
}));

vi.mock('@/stores/storage/workspaces', () => ({
  useWorkspacesStore: () => ({
    init: workspacesInitMock,
    loadCurrentTabGroup: workspacesLoadCurrentTabGroupMock,
    openOrFocusTabGroup: openOrFocusTabGroupMock,
    getDirEntry: getDirEntryMock,
    setPendingLaunchReveal: setPendingLaunchRevealMock,
  }),
}));

vi.mock('@/stores/storage/remote-connections', () => ({
  useRemoteConnectionsStore: () => ({
    init: remoteConnectionsInitMock,
  }),
}));

vi.mock('@/stores/runtime/platform', () => ({
  usePlatformStore: () => ({
    init: platformInitMock,
    appUpdatesManagedExternally: false,
  }),
}));

vi.mock('@/stores/runtime/global-search', () => ({
  useGlobalSearchStore: () => ({
    isOpen: false,
    initOnLaunch: globalSearchInitOnLaunchMock,
    open: vi.fn(),
    close: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/app-window', () => ({
  useAppWindowStore: () => ({
    initMainWindowStateListeners: appWindowInitMainWindowStateListenersMock,
    disposeMainWindowStateListeners: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/shortcuts', () => ({
  BUILTIN_NAVIGATION_PAGE_SHORTCUTS: [
    {
      id: 'switchToHomePage',
      routeName: 'home',
    },
    {
      id: 'switchToNavigatorPage',
      routeName: 'navigator',
    },
    {
      id: 'switchToDashboardPage',
      routeName: 'dashboard',
    },
    {
      id: 'switchToSettingsPage',
      routeName: 'settings',
    },
    {
      id: 'switchToExtensionsPage',
      routeName: 'extensions',
    },
  ],
  useShortcutsStore: () => ({
    init: shortcutsInitMock,
    registerHandler: vi.fn(),
    unregisterHandler: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/global-shortcuts', () => ({
  useGlobalShortcutsStore: () => ({
    init: globalShortcutsInitMock,
  }),
}));

vi.mock('@/stores/runtime/terminals', () => ({
  useTerminalsStore: () => ({
    init: terminalsInitMock,
  }),
}));

vi.mock('@/stores/runtime/background-media', () => ({
  useBackgroundMediaStore: () => ({
    refreshCustomBackgrounds: backgroundMediaRefreshCustomBackgroundsMock,
  }),
}));

vi.mock('@/stores/runtime/extensions', () => ({
  useExtensionsStore: () => ({
    init: extensionsInitMock,
  }),
}));

vi.mock('@/stores/runtime/archive-jobs', () => ({
  useArchiveJobsStore: () => ({
    ensureEventListeners: archiveJobsEnsureEventListenersMock,
  }),
}));

vi.mock('@/stores/runtime/delete-jobs', () => ({
  useDeleteJobsStore: () => ({
    ensureEventListeners: deleteJobsEnsureEventListenersMock,
  }),
}));

vi.mock('@/stores/runtime/copy-move-jobs', () => ({
  useCopyMoveJobsStore: () => ({
    ensureEventListeners: copyMoveJobsEnsureEventListenersMock,
  }),
}));

vi.mock('@/stores/runtime/quick-view', () => ({
  useQuickViewStore: () => ({
    ensureMainWindowDisplayedPathListener: quickViewEnsureMainWindowDisplayedPathListenerMock,
  }),
}));

vi.mock('@/utils/disable-web-view-features', () => ({
  disableWebViewFeatures: disableWebViewFeaturesMock,
}));

vi.mock('@/modules/changelog', () => ({
  useChangelog: () => ({
    checkAndShowChangelog: checkAndShowChangelogMock,
  }),
}));

vi.mock('@/modules/app-updater', () => ({
  useAppUpdater: () => ({
    initAutoCheck: initAutoCheckMock,
  }),
}));

vi.mock('@/composables/use-clipboard-focus-sync', () => ({
  useClipboardFocusSync: vi.fn(),
}));

vi.mock('@/utils/autostart-sync', () => ({
  applyLaunchAtStartupPreference: applyLaunchAtStartupPreferenceMock,
}));

vi.mock('@/utils/auxiliary-windows', () => ({
  prelaunchConfiguredAuxiliaryWindows: vi.fn().mockResolvedValue(undefined),
  setupAuxiliaryWindowLifecycle: vi.fn().mockResolvedValue(vi.fn()),
  acquireAuxiliaryWindow: vi.fn().mockResolvedValue(null),
}));

vi.mock('@/utils/launch-directories', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/utils/launch-directories')>();
  return {
    ...actual,
    resolveLaunchTargetsFromArgs: resolveLaunchTargetsFromArgsMock,
  };
});

vi.mock('@/utils/ui-zoom', () => ({
  applyUiZoomStep: vi.fn(),
}));

vi.mock('@/utils/window-fullscreen', () => ({
  toggleMainWindowFullscreen: vi.fn(),
}));

vi.mock('@/utils/app-splash', () => ({
  removeAppSplash: removeAppSplashMock,
}));

vi.mock('@/utils/open-navigator-directory', () => ({
  preloadNavigatorRoute: vi.fn(),
}));

vi.mock('@/utils/path-comparison-volume-cache', () => ({
  warmPathComparisonVolumeCache: vi.fn().mockResolvedValue(undefined),
}));

// FORK-MODIFICATION 2026-11-19（批次 23）：设备打开失败提示用到的三样东西都换成桩。
//
// `vue-i18n` 必须换：真实 `useI18n()` 要求有活跃的组件实例与 i18n 实例，
// 而这里是纯函数式调用（`useInit()` 由 app.vue 在 setup 里调，但单测直接调）。
vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string) => `t:${key}`,
  }),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    custom: toastCustomMock,
  },
  ToastStatic: { name: 'ToastStatic' },
}));

vi.mock('@/utils/open-in-system-explorer', () => ({
  openInSystemExplorer: openInSystemExplorerMock,
}));

describe('useInit startup restoration', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    appWindowInitMainWindowStateListenersMock.mockReset();
    applyLaunchAtStartupPreferenceMock.mockReset().mockResolvedValue(undefined);
    archiveJobsEnsureEventListenersMock.mockReset().mockResolvedValue(undefined);
    backgroundMediaRefreshCustomBackgroundsMock.mockReset().mockResolvedValue(undefined);
    checkAndShowChangelogMock.mockReset().mockResolvedValue(undefined);
    copyMoveJobsEnsureEventListenersMock.mockReset().mockResolvedValue(undefined);
    deleteJobsEnsureEventListenersMock.mockReset().mockResolvedValue(undefined);
    disableWebViewFeaturesMock.mockReset();
    extensionsInitMock.mockReset().mockResolvedValue(undefined);
    getDirEntryMock.mockReset().mockResolvedValue(null);
    globalSearchInitOnLaunchMock.mockReset().mockResolvedValue(undefined);
    globalShortcutsInitMock.mockReset().mockResolvedValue(undefined);
    hideWindowMock.mockReset().mockResolvedValue(undefined);
    initAutoCheckMock.mockReset().mockResolvedValue(undefined);
    invokeMock.mockReset();
    openOrFocusTabGroupMock.mockReset().mockResolvedValue(undefined);
    platformInitMock.mockReset().mockResolvedValue(undefined);
    quickViewEnsureMainWindowDisplayedPathListenerMock.mockReset().mockResolvedValue(undefined);
    removeAppSplashMock.mockReset();
    remoteConnectionsInitMock.mockReset().mockResolvedValue(undefined);
    resolveLaunchTargetsFromArgsMock.mockReset().mockResolvedValue([]);
    routerPushMock.mockReset().mockResolvedValue(undefined);
    setFocusMock.mockReset().mockResolvedValue(undefined);
    setPendingLaunchRevealMock.mockReset();
    shortcutsInitMock.mockReset().mockResolvedValue(undefined);
    showWindowMock.mockReset().mockResolvedValue(undefined);
    terminalsInitMock.mockReset().mockResolvedValue(undefined);
    userPathsInitMock.mockReset().mockResolvedValue(undefined);
    userSettingsInitMock.mockReset().mockResolvedValue(undefined);
    userStatsInitMock.mockReset().mockResolvedValue(undefined);
    userStatsRunDeferredMaintenanceMock.mockReset().mockResolvedValue(undefined);
    workspacesInitMock.mockReset().mockResolvedValue(undefined);
    workspacesLoadCurrentTabGroupMock.mockReset().mockResolvedValue(undefined);
    userSettingsSetMock.mockReset().mockResolvedValue(undefined);
    userSettingsMock.launchAtStartup = false;
    userSettingsMock.launchAtStartupHidden = false;
    userSettingsMock.startupPage = 'home';
    userSettingsMock.lastRoute = {
      name: 'home',
      fullPageId: '',
    };
    vi.spyOn(performance, 'getEntriesByType').mockReturnValue([
      { type: 'navigate' } as PerformanceNavigationTiming,
    ]);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('loads the saved tab group before deferred launch targets', async () => {
    const callOrder: string[] = [];
    const launchContext = createLaunchContext({
      args: ['sigma-file-manager.exe', 'C:/Launch'],
    });

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return launchContext;
      }

      return null;
    });

    workspacesLoadCurrentTabGroupMock.mockImplementation(async () => {
      callOrder.push('loadCurrentTabGroup');
    });

    openOrFocusTabGroupMock.mockImplementation(async (path: string) => {
      callOrder.push(`openOrFocusTabGroup:${path}`);
    });

    showWindowMock.mockImplementation(async () => {
      callOrder.push('showMainWindow');
    });

    resolveLaunchTargetsFromArgsMock.mockResolvedValue([
      {
        directoryPath: 'C:/Launch',
        focusPath: null,
      },
    ]);

    const { useInit } = await import('@/composables/use-init');
    const { init, awaitBackgroundTasks } = useInit();

    await completeInit(init, awaitBackgroundTasks);

    expect(workspacesInitMock).toHaveBeenCalledWith(undefined, { loadInitialTabGroup: false });
    expect(callOrder.indexOf('loadCurrentTabGroup')).toBeGreaterThan(-1);
    expect(callOrder.indexOf('openOrFocusTabGroup:C:/Launch')).toBeGreaterThan(-1);
    expect(callOrder.indexOf('loadCurrentTabGroup')).toBeLessThan(
      callOrder.indexOf('openOrFocusTabGroup:C:/Launch'),
    );
  });

  it('opens absorbed shell launch targets before the first window show', async () => {
    const callOrder: string[] = [];
    const launchContext = createLaunchContext({
      args: ['sigma-file-manager.exe', 'C:/Launch'],
      hadAbsorbedShellPaths: true,
    });

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return launchContext;
      }

      return null;
    });

    workspacesLoadCurrentTabGroupMock.mockImplementation(async () => {
      callOrder.push('loadCurrentTabGroup');
    });

    openOrFocusTabGroupMock.mockImplementation(async (path: string) => {
      callOrder.push(`openOrFocusTabGroup:${path}`);
    });

    showWindowMock.mockImplementation(async () => {
      callOrder.push('showMainWindow');
    });

    resolveLaunchTargetsFromArgsMock.mockResolvedValue([
      {
        directoryPath: 'C:/Launch',
        focusPath: null,
      },
    ]);

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(workspacesLoadCurrentTabGroupMock).toHaveBeenCalledTimes(1);
    expect(openOrFocusTabGroupMock).toHaveBeenCalledTimes(1);
    expect(showWindowMock).toHaveBeenCalledTimes(1);
    expect(callOrder.indexOf('loadCurrentTabGroup')).toBeLessThan(callOrder.indexOf('showMainWindow'));
    expect(callOrder.indexOf('openOrFocusTabGroup:C:/Launch')).toBeLessThan(
      callOrder.indexOf('showMainWindow'),
    );
  });

  /**
   * FORK-MODIFICATION 2026-10-20：远程连接必须在**启动序列**里加载，不能靠组件
   * 各自 onMounted。
   *
   * 打包版实测到的故障：直接进导航器、没打开过侧栏远程面板和设置页时，
   * `connections` 一直是空的，于是右键菜单的「权限设置」按协议判定整个消失，
   * 目录树根的标签也从连接名回落成 32 位连接 id —— 两处都不报错，
   * 只是功能「凭空没了」。
   *
   * 断言的是**在窗口第一次显示之前**就加载完：用户能看见界面时，
   * 数据必须已经在那里，不能靠「哪个组件先挂载」。
   */
  it('loads remote connections before the main window is shown', async () => {
    const callOrder: string[] = [];

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return createLaunchContext({});
      }

      return null;
    });

    remoteConnectionsInitMock.mockImplementation(async () => {
      callOrder.push('remoteConnectionsInit');
    });

    showWindowMock.mockImplementation(async () => {
      callOrder.push('showMainWindow');
    });

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(remoteConnectionsInitMock).toHaveBeenCalledTimes(1);
    expect(callOrder.indexOf('remoteConnectionsInit')).toBeLessThan(
      callOrder.indexOf('showMainWindow'),
    );
  });

  it('removes the app splash after init completes', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return createLaunchContext({});
      }

      return null;
    });

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(showWindowMock).toHaveBeenCalledTimes(1);
    expect(setFocusMock).toHaveBeenCalledTimes(1);
    expect(hideWindowMock).not.toHaveBeenCalled();
    expect(removeAppSplashMock).toHaveBeenCalledTimes(1);
  });

  it('does not focus the main window after a webview reload', async () => {
    vi.mocked(performance.getEntriesByType).mockReturnValue([
      { type: 'reload' } as PerformanceNavigationTiming,
    ]);

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return createLaunchContext({});
      }

      return null;
    });

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(showWindowMock).toHaveBeenCalledTimes(1);
    expect(setFocusMock).not.toHaveBeenCalled();
    expect(removeAppSplashMock).toHaveBeenCalledTimes(1);
  });

  it('passes a timeout when refreshing custom backgrounds during startup', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return createLaunchContext({});
      }

      return null;
    });

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(backgroundMediaRefreshCustomBackgroundsMock).toHaveBeenCalledTimes(1);
    expect(backgroundMediaRefreshCustomBackgroundsMock).toHaveBeenCalledWith(
      expect.objectContaining({ timeoutMs: expect.any(Number) }),
    );
  });

  it('passes a directory-entry timeout when loading the saved tab group on startup', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return createLaunchContext({});
      }

      return null;
    });

    const { useInit } = await import('@/composables/use-init');
    const { init, awaitBackgroundTasks } = useInit();

    await completeInit(init, awaitBackgroundTasks);

    expect(workspacesLoadCurrentTabGroupMock).toHaveBeenCalledWith(
      expect.objectContaining({ dirEntryTimeoutMs: expect.any(Number) }),
    );
  });

  it('passes a directory-entry timeout when resolving launch targets', async () => {
    const launchContext = createLaunchContext({
      args: ['sigma-file-manager.exe', 'C:/Launch'],
    });

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return launchContext;
      }

      return null;
    });

    resolveLaunchTargetsFromArgsMock.mockImplementation(async (
      _context: unknown,
      resolveDirEntry: (path: string) => Promise<unknown>,
    ) => {
      await resolveDirEntry('C:/Launch');
      return [];
    });

    const { useInit } = await import('@/composables/use-init');
    const { init, awaitBackgroundTasks } = useInit();

    await completeInit(init, awaitBackgroundTasks);

    expect(getDirEntryMock).toHaveBeenCalledWith(
      expect.objectContaining({
        path: 'C:/Launch',
        timeoutMs: expect.any(Number),
      }),
    );
  });

  it('hides the main window when launched only with delegated shell paths', async () => {
    const launchContext = createLaunchContext({
      args: ['sigma-file-manager.exe'],
      hadDelegatedShellPaths: true,
    });

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return launchContext;
      }

      return null;
    });

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(hideWindowMock).toHaveBeenCalledTimes(1);
    expect(showWindowMock).not.toHaveBeenCalled();
    expect(removeAppSplashMock).toHaveBeenCalledTimes(1);
  });

  it('removes the app splash when init fails', async () => {
    platformInitMock.mockRejectedValue(new Error('Platform init failed'));

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(removeAppSplashMock).toHaveBeenCalledTimes(1);
    expect(showWindowMock).not.toHaveBeenCalled();
  });

  it('schedules background startup work after init resolves', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return createLaunchContext({});
      }

      return null;
    });

    const callOrder: string[] = [];

    globalSearchInitOnLaunchMock.mockImplementation(async () => {
      callOrder.push('globalSearch');
    });

    const { useInit } = await import('@/composables/use-init');
    const { init, awaitBackgroundTasks } = useInit();

    const initPromise = init();
    await vi.runOnlyPendingTimersAsync();
    await initPromise;

    expect(callOrder).toEqual([]);

    await vi.runAllTimersAsync();
    const backgroundTasksPromise = awaitBackgroundTasks();
    await drainBackgroundWork(backgroundTasksPromise);
    await backgroundTasksPromise;

    expect(callOrder).toContain('globalSearch');
  });

  it('does not change the route when the startup page is Home', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return createLaunchContext({});
      }

      return null;
    });

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(routerPushMock).not.toHaveBeenCalled();
  });

  it('opens the configured startup page before the first window show', async () => {
    userSettingsMock.startupPage = 'dashboard';
    const callOrder: string[] = [];

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return createLaunchContext({});
      }

      return null;
    });

    routerPushMock.mockImplementation(async () => {
      callOrder.push('routerPush');
    });

    showWindowMock.mockImplementation(async () => {
      callOrder.push('showMainWindow');
    });

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(routerPushMock).toHaveBeenCalledWith({ name: 'dashboard' });
    expect(callOrder.indexOf('routerPush')).toBeLessThan(callOrder.indexOf('showMainWindow'));
  });

  it('does not apply the startup page when launch args open a folder', async () => {
    userSettingsMock.startupPage = 'dashboard';
    userSettingsMock.lastRoute = {
      name: 'settings',
      fullPageId: '',
    };

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return createLaunchContext({
          args: ['sigma-file-manager.exe', 'C:/Launch'],
        });
      }

      return null;
    });

    resolveLaunchTargetsFromArgsMock.mockResolvedValue([
      {
        directoryPath: 'C:/Launch',
        focusPath: null,
      },
    ]);

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(routerPushMock).not.toHaveBeenCalledWith({ name: 'dashboard' });
    expect(userSettingsSetMock).not.toHaveBeenCalledWith('lastRoute', {
      name: 'home',
      fullPageId: '',
    });
  });

  it('restores the last page when that startup option is selected', async () => {
    userSettingsMock.startupPage = 'last';
    userSettingsMock.lastRoute = {
      name: 'settings',
      fullPageId: '',
    };

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_launch_context') {
        return createLaunchContext({});
      }

      return null;
    });

    const { useInit } = await import('@/composables/use-init');
    const { init } = useInit();

    await completeInit(init);

    expect(routerPushMock).toHaveBeenCalledWith({ name: 'settings' });
  });

  // FORK-MODIFICATION 2026-11-19（批次 23）：设备（手机 / MTP）的启动落点。
  //
  // 故障链：手机传给 AFM 的 argv 是 `::{20D04FE0-...}\?\usb#...`。
  // 改动前它被规范化改坏 ⇒ 候选被判成空 ⇒ 兜底把用户丢到「位置」页。
  // Rust 侧改动后这类路径**原样留在 args 里**，前端这边必须真的导航到设备，
  // 设备打不开时必须给用户**明确可见**的提示，而不是静默停在启动页。
  describe('shell 设备启动', () => {
    const devicePath = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\\usb#vid_22d9&pid_2765';

    function createDeviceLaunchContext() {
      return createLaunchContext({
        args: ['sigma-file-manager.exe', devicePath],
        hadAbsorbedShellPaths: false,
        hadDelegatedShellPaths: false,
      });
    }

    beforeEach(() => {
      toastCustomMock.mockReset();
      openInSystemExplorerMock.mockReset().mockResolvedValue(undefined);
    });

    it('有 shell 候选时真的导航到设备，而不是落到「位置」页', async () => {
      invokeMock.mockImplementation(async (command: string) => {
        if (command === 'get_launch_context') {
          return createDeviceLaunchContext();
        }

        return null;
      });

      getDirEntryMock.mockResolvedValue({
        name: 'Galaxy S22',
        path: devicePath,
        is_file: false,
        is_dir: true,
      });

      resolveLaunchTargetsFromArgsMock.mockResolvedValue([
        {
          directoryPath: devicePath,
          focusPath: null,
        },
      ]);

      const { useInit } = await import('@/composables/use-init');
      const { init, awaitBackgroundTasks } = useInit();

      await completeInit(init, awaitBackgroundTasks);

      expect(openOrFocusTabGroupMock).toHaveBeenCalledWith(devicePath);
      // 「位置」页兜底没有顶掉设备候选。
      expect(openOrFocusTabGroupMock).not.toHaveBeenCalledWith('sfm://locations');
      expect(routerPushMock).toHaveBeenCalledWith({ name: 'navigator' });
      expect(toastCustomMock).not.toHaveBeenCalled();
    });

    it('设备打不开时给用户明确可见的提示，并给出「在系统资源管理器中打开」的动作', async () => {
      invokeMock.mockImplementation(async (command: string) => {
        if (command === 'get_launch_context') {
          return createDeviceLaunchContext();
        }

        return null;
      });

      // 设备已拔出 ⇒ 拿不到条目 ⇒ 解析不出启动目标。
      getDirEntryMock.mockResolvedValue(null);
      resolveLaunchTargetsFromArgsMock.mockResolvedValue([]);

      const { useInit } = await import('@/composables/use-init');
      const { init, awaitBackgroundTasks } = useInit();

      await completeInit(init, awaitBackgroundTasks);

      expect(toastCustomMock).toHaveBeenCalledTimes(1);

      const toastOptions = toastCustomMock.mock.calls[0]?.[1] as
        { componentProps?: { data?: Record<string, unknown> } }
        | undefined;
      const toastData = toastOptions?.componentProps?.data;

      expect(toastData?.title).toBe('t:fileBrowser.deviceOpenFailed');
      expect(toastData?.description).toBe('t:fileBrowser.deviceOpenFailedDescription');
      expect(toastData?.actionText).toBe('t:fileBrowser.openInSystemExplorer');

      // 点了动作就真的交给系统资源管理器。
      const onAction = toastData?.onAction as (() => void) | undefined;

      onAction?.();

      expect(openInSystemExplorerMock).toHaveBeenCalledWith(devicePath);
    });

    it('hadDelegatedShellPaths 为 false 时窗口照常显示，不被隐藏', async () => {
      invokeMock.mockImplementation(async (command: string) => {
        if (command === 'get_launch_context') {
          return createDeviceLaunchContext();
        }

        return null;
      });

      getDirEntryMock.mockResolvedValue(null);
      resolveLaunchTargetsFromArgsMock.mockResolvedValue([]);

      const { useInit } = await import('@/composables/use-init');
      const { init } = useInit();

      await completeInit(init);

      expect(showWindowMock).toHaveBeenCalledTimes(1);
      expect(hideWindowMock).not.toHaveBeenCalled();
    });
  });
});

function createLaunchContext(overrides: Partial<{
  args: string[];
  cwd: string | null;
  executableDir: string | null;
  hadAbsorbedShellPaths: boolean;
  hadDelegatedShellPaths: boolean;
}>) {
  return {
    args: ['sigma-file-manager.exe'],
    cwd: null,
    executableDir: null,
    hadAbsorbedShellPaths: false,
    hadDelegatedShellPaths: false,
    ...overrides,
  };
}

async function completeInit(
  init: () => Promise<void>,
  awaitBackgroundTasks?: () => Promise<void>,
) {
  const initPromise = init();
  await vi.runOnlyPendingTimersAsync();
  await initPromise;

  if (!awaitBackgroundTasks) {
    return;
  }

  await vi.runAllTimersAsync();
  const backgroundTasksPromise = awaitBackgroundTasks();
  await drainBackgroundWork(backgroundTasksPromise);
  await backgroundTasksPromise;
}

async function drainBackgroundWork(settledPromise: Promise<unknown>) {
  for (let attemptIndex = 0; attemptIndex < 50; attemptIndex += 1) {
    await Promise.resolve();
    await vi.runOnlyPendingTimersAsync();

    const settled = await Promise.race([
      settledPromise.then(() => true, () => true),
      Promise.resolve(false),
    ]);

    if (settled) {
      return;
    }
  }
}
