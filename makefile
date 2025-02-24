# Copyright 2025 Mist Tecnologia LTDA. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

HOST_PLATFORM := $(shell uname -s | tr '[:upper:]' '[:lower:]')
HOST_ARCH := $(shell uname -m)

ifeq ($(HOST_ARCH),x86_64)
HOST_ARCH = x64
else ifeq ($(HOST_ARCH),aarch64)
HOST_ARCH = arm64
else
$(error Unsupported host architecture: $(HOST_ARCH))
endif

ifeq ($(HOST_PLATFORM),linux)
HOST_OS = linux
else ifeq ($(HOST_PLATFORM),darwin)
HOST_OS = mac
else
$(error Unsupported host platform: $(HOST_PLATFORM))
endif

# Build variables
MKROOT ?= $(PWD)
OUTPUT ?= out/default
NOECHO ?= @
GN ?= $(MKROOT)/prebuilt/third_party/gn/$(HOST_OS)-$(HOST_ARCH)/gn
NINJA ?= $(MKROOT)/prebuilt/third_party/ninja/$(HOST_OS)-$(HOST_ARCH)/ninja

# By default, also show the number of actively running actions.
export NINJA_STATUS="[%f/%t][%p/%w](%r) "
# By default, print the 4 oldest commands that are still running.
export NINJA_STATUS_MAX_COMMANDS=4
export NINJA_STATUS_REFRESH_MILLIS=100
export NINJA_PERSISTENT_MODE=0

gen: ## Generate ninja
	$(NOECHO)echo "Running:$(GN) gen $(OUTPUT)"
	$(NOECHO)$(GN) gen $(OUTPUT)
.PHONY: gen

args: ## Set up build dir and arguments file
	$(NOECHO)mkdir -p $(OUTPUT)
	$(NOECHO)echo "# Basic args:" > $(OUTPUT)/args.gn
.PHONY: args

debug: args ## Set debug arguments
	$(NOECHO)echo "compilation_mode = \"debug\"" >> $(OUTPUT)/args.gn
.PHONY: debug

gdb: args ## Set debug arguments
	$(NOECHO)echo "compress_debuginfo = \"none\"" >> $(OUTPUT)/args.gn
	$(NOECHO)echo "optimize = \"debug\"" >> $(OUTPUT)/args.gn
	$(NOECHO)echo "mk_optimize = \"debug\"" >> $(OUTPUT)/args.gn
	$(NOECHO)echo "kernel_extra_defines = [ \"DISABLE_KASLR\" ]" >> $(OUTPUT)/args.gn
.PHONY: gdb

spotless:
	rm -rf -- "$(MKROOT)/$(OUTPUT)"

%: ## Make any ninja target
	$(NOECHO)$(NINJA) -C $(OUTPUT) $@