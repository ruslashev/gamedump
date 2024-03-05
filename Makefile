NAME = main
MODE = debug

TARGET_DIR = $(shell pwd)/target
BUILT_SHADERS_DIR = $(TARGET_DIR)/shaders
BIN = $(TARGET_DIR)/$(MODE)/$(NAME)
DEP = $(BIN).d

SHADERS = $(wildcard shaders/*)
BUILT_SHADERS = $(SHADERS:shaders/%=$(BUILT_SHADERS_DIR)/%.spv)
GLSLC_FLAGS = -O

ASSETS = cat.jpg
BUILT_ASSETS = $(ASSETS:%.jpg=assets/%.jxl)
CJXL_FLAGS = --effort 10 --brotli_effort 11 --lossless_jpeg=1 --premultiply=1 \
             --keep_invisible=0 --num_threads=-1 --jpeg_store_metadata=0 \
             --allow_expert_options --quiet

ifeq ($(MODE), release)
    CFLAGS = --release
else ifeq ($(MODE), debug)
    BFLAGS = mold -run
else
    $(error Unknown build mode "$(MODE)")
endif

run: $(BIN)
	@$(BIN)

all: $(BIN)

release:
	@$(MAKE) MODE=release -s all

shaders: $(BUILT_SHADERS)

assets: $(BUILT_ASSETS)

$(BIN): $(BUILT_SHADERS) $(BUILT_ASSETS)
	$(BFLAGS) cargo build $(CFLAGS)

$(BUILT_SHADERS_DIR)/%.spv: shaders/%
	@mkdir -p $(@D)
	@echo glslc $^
	@glslc $(GLSLC_FLAGS) $^ -o $@

assets/%.jxl: assets/original/%.jpg
	@echo cjxl $@
	@cjxl $(CJXL_FLAGS) $^ $@

clippy:
	cargo clippy -- -W clippy::pedantic -W clippy::nursery -W clippy::unwrap_used

gdb: $(BIN)
	gdb $(BIN) -ex run

valgrind: $(BIN)
	valgrind --leak-check=full $(BIN)

clean:
	cargo clean
	@rm -f $(BUILT_ASSETS)

clean-shaders:
	@rm -f $(BUILT_SHADERS)

clean-assets:
	@rm -f $(BUILT_ASSETS)

.NOTPARALLEL:
.PHONY: run all release shaders assets clippy gdb valgrind clean clean-shaders clean-assets

-include $(DEP)
