// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { markRaw, toRaw } from 'vue';
import type { Component } from 'vue';
import { toast as sonnerToast } from 'vue-sonner';
import type { ExternalToast } from 'vue-sonner';
import ToastStatic from './toast-static.vue';
import ToastProgress from './toast-progress.vue';

const allowedCustomComponents = new Set<Component>([ToastStatic, ToastProgress]);

function isAllowedCustomComponent(component: Component): boolean {
  const resolved = toRaw(component);
  return allowedCustomComponents.has(resolved);
}

// The public `toast()` accepts two shapes, because both are in active use in
// this codebase and sonner itself accepts both:
//   - toast('标题', { description })      -> title is the first argument
//   - toast({ title, description })       -> title lives inside the object
// Normalizing here keeps the object shape from reaching toast-header.vue,
// which expects a string title (it calls String#indexOf on it).
type ToastInput
  = | string
    | {
      title?: unknown;
      description?: unknown;
      [key: string]: unknown;
    };

function staticDescription(data?: ExternalToast): string {
  if (!data?.description) {
    return '';
  }

  return typeof data.description === 'string' ? data.description : '';
}

function normalizeInput(
  message: ToastInput,
  data?: ExternalToast,
): {
  title: string;
  description: string;
  rest: Record<string, unknown>;
} {
  const rest: Record<string, unknown> = data ? { ...data } : {};

  if (typeof message === 'string') {
    return {
      title: message,
      description: staticDescription(data),
      rest,
    };
  }

  const title = typeof message?.title === 'string' ? message.title : '';
  const description
    = typeof message?.description === 'string' ? message.description : staticDescription(data);

  for (const key of Object.keys(message ?? {})) {
    if (key === 'title' || key === 'description') {
      continue;
    }

    rest[key] = (message as Record<string, unknown>)[key];
  }

  return {
    title,
    description,
    rest,
  };
}

function showStaticToast(message: ToastInput, data?: ExternalToast) {
  const { title, description, rest } = normalizeInput(message, data);
  delete rest.description;
  delete rest.componentProps;

  return sonnerToast.custom(markRaw(ToastStatic), {
    ...(rest as ExternalToast),
    componentProps: {
      data: {
        title,
        description,
      },
    },
  });
}

export const toast = Object.assign(
  (message: ToastInput, data?: ExternalToast) => showStaticToast(message, data),
  {
    custom: (component: Component, options?: ExternalToast) => {
      if (!isAllowedCustomComponent(component)) {
        throw new Error(
          'toast.custom only accepts ToastStatic or ToastProgress from @/components/ui/toaster.',
        );
      }

      return sonnerToast.custom(markRaw(component), options);
    },
    dismiss: sonnerToast.dismiss.bind(sonnerToast),
    promise: sonnerToast.promise.bind(sonnerToast),
    getHistory: sonnerToast.getHistory.bind(sonnerToast),
    getToasts: sonnerToast.getToasts.bind(sonnerToast),
    error: (title: string, data?: ExternalToast) => showStaticToast(title, data),
    success: (title: string, data?: ExternalToast) => showStaticToast(title, data),
    warning: (title: string, data?: ExternalToast) => showStaticToast(title, data),
    info: (title: string, data?: ExternalToast) => showStaticToast(title, data),
    loading: (title: string, data?: ExternalToast) => showStaticToast(title, data),
    message: (title: string, data?: ExternalToast) => showStaticToast(title, data),
  },
) as unknown as typeof sonnerToast;
