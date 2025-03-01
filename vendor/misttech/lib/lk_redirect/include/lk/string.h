/*
 * Copyright (c) 2008 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
#pragma once

#include <stddef.h>
#include <zircon/compiler.h>

__BEGIN_CDECLS

char *strdup(const char *str) __MALLOC;

__END_CDECLS
