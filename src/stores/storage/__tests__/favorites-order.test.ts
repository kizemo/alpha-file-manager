// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 2026-10-31 —— 收藏夹排序的纯函数单测。
//
// 这层是「拖拽」和「上移 / 下移」两条交互路径共同的语义来源，
// 所以单独测它：只要这里对，界面层怎么调都不会写出错序。

import { describe, expect, it } from 'vitest';
import { reorderByDrop, reorderFavoriteCategoriesByDrop, reorderFavoritesByDrop } from '@/stores/storage/utils/favorites-order';
import type { FavoriteCategory, FavoriteItem } from '@/types/user-stats';

function createFavorite(path: string, categoryId: string | null = null): FavoriteItem {
  return {
    path,
    addedAt: 1,
    isFile: false,
    categoryId,
  };
}

function createCategory(id: string): FavoriteCategory {
  return {
    id,
    name: id,
    createdAt: 1,
  };
}

describe('reorderByDrop', () => {
  it('moves the dragged item in front of the target', () => {
    const items = ['a', 'b', 'c', 'd'];

    expect(reorderByDrop(items, 0, 2, 'before')).toEqual(['b', 'a', 'c', 'd']);
  });

  it('moves the dragged item behind the target', () => {
    const items = ['a', 'b', 'c', 'd'];

    expect(reorderByDrop(items, 0, 2, 'after')).toEqual(['b', 'c', 'a', 'd']);
  });

  it('moves the dragged item backwards across several items', () => {
    const items = ['a', 'b', 'c', 'd'];

    expect(reorderByDrop(items, 3, 1, 'before')).toEqual(['a', 'd', 'b', 'c']);
  });

  it('leaves the list untouched for out-of-range indexes', () => {
    const items = ['a', 'b'];

    expect(reorderByDrop(items, -1, 0, 'before')).toBe(items);
    expect(reorderByDrop(items, 0, 5, 'before')).toBe(items);
  });

  it('leaves the list untouched when the target is the dragged item itself', () => {
    const items = ['a', 'b'];

    expect(reorderByDrop(items, 1, 1, 'after')).toBe(items);
  });
});

describe('reorderFavoritesByDrop', () => {
  it('reorders two favorites of the same category', () => {
    const favorites = [
      createFavorite('/a', 'cat-1'),
      createFavorite('/b', 'cat-1'),
      createFavorite('/c', 'cat-1'),
    ];

    const next = reorderFavoritesByDrop(favorites, '/a', '/c', 'before');

    expect(next.map(item => item.path)).toEqual(['/b', '/a', '/c']);
  });

  it('keeps the order of the other categories untouched', () => {
    const favorites = [
      createFavorite('/w1', 'cat-1'),
      createFavorite('/w2', 'cat-1'),
      createFavorite('/h1', 'cat-2'),
    ];

    const next = reorderFavoritesByDrop(favorites, '/w1', '/w2', 'after');

    expect(next.map(item => item.path)).toEqual(['/w2', '/w1', '/h1']);
  });

  it('refuses to move a favorite into another category', () => {
    const favorites = [
      createFavorite('/w1', 'cat-1'),
      createFavorite('/h1', 'cat-2'),
    ];

    expect(reorderFavoritesByDrop(favorites, '/w1', '/h1', 'before')).toBe(favorites);
  });

  it('refuses to reorder an unknown path', () => {
    const favorites = [createFavorite('/a', 'cat-1')];

    expect(reorderFavoritesByDrop(favorites, '/missing', '/a', 'before')).toBe(favorites);
  });
});

describe('reorderFavoriteCategoriesByDrop', () => {
  it('moves a category in front of the target category', () => {
    const categories = [
      createCategory('a'),
      createCategory('b'),
      createCategory('c'),
    ];

    const next = reorderFavoriteCategoriesByDrop(categories, 'c', 'a', 'before');

    expect(next.map(category => category.id)).toEqual(['c', 'a', 'b']);
  });

  it('moves a category behind the target category', () => {
    const categories = [
      createCategory('a'),
      createCategory('b'),
      createCategory('c'),
    ];

    const next = reorderFavoriteCategoriesByDrop(categories, 'a', 'c', 'after');

    expect(next.map(category => category.id)).toEqual(['b', 'c', 'a']);
  });

  it('refuses to reorder an unknown category', () => {
    const categories = [createCategory('a')];

    expect(reorderFavoriteCategoriesByDrop(categories, 'missing', 'a', 'before')).toBe(categories);
  });
});
