// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: e2e placeholder for tree view (issue #499)
// Toggle UI landed 2026-09-25 (feat/tree-view toolbar dropdown). Still skipped — assertions pending (out of scope of toolbar-toggle plan).

import { browser } from '@wdio/globals';

describe('Navigator tree view (placeholder)', () => {
  it.skip('expands a directory when its row is clicked', async () => {
    await browser.url('/');
    // TODO: open navigator + switch layout to tree + click first row + assert aria-expanded
  });

  it.skip('navigates to a file directory on activation', async () => {
    await browser.url('/');
    // TODO: open navigator + tree layout + expand path + activate leaf + assert breadcrumb
  });
});