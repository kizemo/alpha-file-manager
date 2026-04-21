// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

const SPLASH_ELEMENT_ID = 'app-splash';
const SPLASH_HIDDEN_CLASS = 'app-splash--hidden';
const SPLASH_REMOVAL_FALLBACK_MS = 500;
const INIT_TRACE_LOG_PREFIX = '[init-trace]';

function formatElapsed(elapsedMs: number): string {
  if (elapsedMs >= 1000) {
    return `${(elapsedMs / 1000).toFixed(2)}s`;
  }

  return `${Math.round(elapsedMs)}ms`;
}

export function logInitTrace(message: string): void {
  console.log(`${INIT_TRACE_LOG_PREFIX} ${message}`);
}

export async function traceInitStep<TResult>(
  stepLabel: string,
  step: () => Promise<TResult>,
): Promise<TResult> {
  const startTime = performance.now();
  logInitTrace(`-> ${stepLabel}`);

  try {
    const stepResult = await step();
    const elapsedMs = performance.now() - startTime;
    logInitTrace(`   done ${stepLabel} (${formatElapsed(elapsedMs)})`);
    return stepResult;
  }
  catch (stepError) {
    const elapsedMs = performance.now() - startTime;
    console.error(
      `${INIT_TRACE_LOG_PREFIX}    FAIL ${stepLabel} (${formatElapsed(elapsedMs)}):`,
      stepError,
    );
    throw stepError;
  }
}

export function removeAppSplash(): void {
  if (typeof document === 'undefined') {
    return;
  }

  const splashElement = document.getElementById(SPLASH_ELEMENT_ID);

  if (!splashElement) {
    return;
  }

  splashElement.classList.add(SPLASH_HIDDEN_CLASS);

  function removeElement() {
    splashElement?.remove?.();
  }

  splashElement.addEventListener('transitionend', removeElement, { once: true });
  setTimeout(removeElement, SPLASH_REMOVAL_FALLBACK_MS);
}
