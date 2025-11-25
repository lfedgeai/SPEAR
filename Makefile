
VERSION := $(shell git describe --tags --match "*" --always --dirty)
REPO_ROOT := $(shell pwd)
OUTPUT_DIR := $(REPO_ROOT)/bin

HOST_UNAME_M := $(shell uname -m)
GO_DEFAULT_OS := $(shell go env GOOS)
GO_DEFAULT_ARCH := $(shell go env GOARCH)

# Allow overriding the workload build platform (e.g. linux/arm64)
WORKLOAD_PLATFORM ?= $(GO_DEFAULT_OS)/$(GO_DEFAULT_ARCH)

FLATC := $(shell command -v flatc 2> /dev/null)

ifndef FLATC
  $(error "flatc binary not found in PATH. Please install flatc.")
endif

all: clean spearlet workload sdk


SUBDIRS := $(shell find $(REPO_ROOT) -mindepth 1 -maxdepth 3 -type d -exec test -e {}/Makefile \; -exec echo {} \; | grep -v spear-next)
WORKLOAD_SUBDIRS := $(shell find $(REPO_ROOT)/workload -mindepth 1 -maxdepth 3 -type d -exec test -e {}/Makefile \; -exec echo {} \;)

clean:
	@set -ex; \
	docker system prune -f || true && \
	rm -rf $(OUTPUT_DIR) && \
	rm -rf $(REPO_ROOT)/pkg/spear && \
	for dir in $(SUBDIRS); do \
		make -C $$dir clean; \
	done

build: spearlet
	@set -e; \
	for dir in $(SUBDIRS); do \
		make -C $$dir build; \
	done

install_sdk: build
	@set -e; \
	cd $(REPO_ROOT)/sdk/python && \
	file=$$(printf "%s\n" ./dist/spear-*.whl | head -n1); \
	python -m pip install "$$file" --force-reinstall

spearlet: pkg/spear
	mkdir -p $(OUTPUT_DIR)
	go build -o $(OUTPUT_DIR)/spearlet \
	-ldflags "-X 'github.com/lfedgeai/spear/pkg/common.Version=$(VERSION)'" \
	$(REPO_ROOT)/cmd/spearlet/main.go

spearlet-linux-arm64: pkg/spear
	mkdir -p $(OUTPUT_DIR)/linux-arm64
	GOOS=linux GOARCH=arm64 CGO_ENABLED=1 go build -o $(OUTPUT_DIR)/linux-arm64/spearlet \
	-ldflags "-X 'github.com/lfedgeai/spear/pkg/common.Version=$(VERSION)'" \
	$(REPO_ROOT)/cmd/spearlet/main.go

test: workload build install_sdk
	@set -e; \
	go test -v ./test/... && \
	for dir in $(SUBDIRS); do \
		make -C $$dir test; \
	done

workload: build
	@set -e; \
	for dir in $(WORKLOAD_SUBDIRS); do \
		$(MAKE) -C $$dir PLATFORM=$(WORKLOAD_PLATFORM); \
	done

workload-linux-arm64:
	@$(MAKE) WORKLOAD_PLATFORM=linux/arm64 workload

workload-linux-amd64:
	@$(MAKE) WORKLOAD_PLATFORM=linux/amd64 workload

format_python:
	black $(REPO_ROOT)/; \
	isort $(REPO_ROOT)/

format_golang:
	gofmt -w .

format: format_python format_golang

pkg/spear:
	allfiles=`find ${REPO_ROOT}/proto -name "*.fbs"`; \
	go_module_flag=""; \
	if $(FLATC) --help 2>/dev/null | grep -q -- "--go-module-name"; then \
		go_module_flag="--go-module-name github.com/lfedgeai/spear/pkg"; \
	fi; \
	$(FLATC) -o $(REPO_ROOT)/pkg/ -I ${REPO_ROOT}/proto $$go_module_flag --go --gen-all $${allfiles}

.PHONY: all spearlet spearlet-linux-arm64 test workload workload-linux-arm64 workload-linux-amd64 clean format_python format
