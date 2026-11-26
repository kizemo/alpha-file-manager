// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// Guards the repo-relative paths that the focus-sync installer scripts use for
// the sandbox gate. Those literals silently rotted twice: when the plugin
// source was merged into this repo, and again during the 2026-11-10 root
// unification. A stale literal does not throw - the gate just resolves to a
// non-existent path and is SKIPPED, so the extension can be deployed even
// though the app would reject it (silent never-load).

import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

// Vitest does not give the test file a file:// URL (import.meta.url is served
// by the module runner), so the repository root is resolved from the working
// directory vitest was started in - the project root.
const repoRoot = path.resolve(process.cwd());
const installerDir = path.join(repoRoot, 'extensions', 'kizemo.focus-sync', 'installer');

if (!fs.existsSync(installerDir)) {
  throw new Error(`cannot locate the focus-sync installer folder from ${repoRoot}`);
}

function readInstaller(name: string): string {
  return fs.readFileSync(path.join(installerDir, name), 'utf8');
}

// Pulls the literal handed to Join-Path for one variable, e.g.
//   $sandboxScanner = Join-Path $ScriptDir '..\..\..\scripts\scan-sandbox-dynamic.cjs'
function joinPathLiteral(script: string, variable: string): string {
  const pattern = new RegExp(`\\$${variable}\\s*=\\s*Join-Path\\s+\\$\\w+\\s+'([^']+)'`);
  const matched = script.match(pattern);

  expect(matched, `no Join-Path literal found for $${variable}`).not.toBeNull();

  const literal = matched ? matched[1] : '';

  expect(literal, `$${variable} literal is empty`).not.toBe('');

  return literal;
}

describe('focus-sync installer sandbox gate paths', () => {
  const scripts = ['register.ps1', 'manual-install.ps1'];

  for (const name of scripts) {
    it(`${name} resolves the scanner and the sandbox rules to files that exist`, () => {
      const script = readInstaller(name);
      const scanner = joinPathLiteral(script, 'sandboxScanner');
      const sandboxRules = joinPathLiteral(script, 'sandboxTs');

      expect(fs.existsSync(path.resolve(installerDir, scanner))).toBe(true);
      expect(fs.existsSync(path.resolve(installerDir, sandboxRules))).toBe(true);
    });
  }

  it('keeps the extension source inside the extension folder that actually ships', () => {
    const script = readInstaller('manual-install.ps1');
    const sourceDir = script.match(/\$srcDir\s*=\s*Join-Path\s+\$ScriptDir\s+'([^']+)'/);
    const literal = sourceDir ? sourceDir[1] : '';
    const resolved = path.resolve(installerDir, literal);

    expect(fs.existsSync(path.join(resolved, 'package.json'))).toBe(true);
    expect(fs.existsSync(path.join(resolved, 'dist', 'index.js'))).toBe(true);
    expect(fs.existsSync(path.join(resolved, 'bin', 'focus-sync-sidecar.exe'))).toBe(true);
  });
});
