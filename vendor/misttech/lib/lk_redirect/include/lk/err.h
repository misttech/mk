// Copyright 2025 Mist Tecnologia Ltda. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef VENDOR_MISTTECH_LIB_LK_REDIRECT_INCLUDE_LK_ERR_H_
#define VENDOR_MISTTECH_LIB_LK_REDIRECT_INCLUDE_LK_ERR_H_

#include <zircon/errors.h>

#define NO_ERROR ZX_OK
#define ERR_NOT_FOUND ZX_ERR_NOT_FOUND
#define ERR_NO_RESOURCES ZX_ERR_NO_RESOURCES
#define ERR_NOT_CONFIGURED ZX_ERR_UNAVAILABLE
#define ERR_NOT_SUPPORTED ZX_ERR_NOT_SUPPORTED
#define ERR_INVALID_ARGS ZX_ERR_INVALID_ARGS
#define ERR_NOT_VALID ZX_ERR_BAD_HANDLE
#define ERR_NO_MEMORY ZX_ERR_NO_MEMORY
#define ERR_IO ZX_ERR_IO

#endif  // VENDOR_MISTTECH_LIB_LK_REDIRECT_INCLUDE_LK_ERR_H_
