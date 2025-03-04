# Copyright 2022 The Fuchsia Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import functools
import os
from typing import Any

from assembly.common import FileEntry

__all__ = [
    "create_fast_copy_mock_instance",
    "fast_copy_mock",
    "mock_fast_copy_in",
]

FilePath = str | os.PathLike[str]


def fast_copy_mock(
    src: FilePath, dst: FilePath, tracked_copies: list[FileEntry]
) -> FilePath | None:
    """A bindable-mock of assembly.fast_copy() that tracks all of the copies
    that it's asked to perform in the passed-in list.
    """
    tracked_copies.append(FileEntry(source=src, destination=dst))
    return src


def create_fast_copy_mock_instance() -> (
    tuple[functools.partial[FilePath | None], list[FileEntry]]
):
    """Create a mock implementation of fast_copy() that's bound to a list of
    FileEntries in which it records all the copies it's asked to make.
    """
    copies: list[FileEntry] = []
    return (functools.partial(fast_copy_mock, tracked_copies=copies), copies)


def mock_fast_copy_in(
    context: Any,
    mock: tuple[functools.partial[FilePath | None], list[FileEntry]]
    | None = None,
) -> tuple[functools.partial[FilePath | None], list[FileEntry]]:
    """Insert a new mock of `fast_copy` into the context, and return it."""
    if mock:
        (mock_instance, copies) = mock
    else:
        (mock_instance, copies) = create_fast_copy_mock_instance()
    context.fast_copy = mock_instance
    context.fast_copy_makedirs = mock_instance
    return (mock_instance, copies)
