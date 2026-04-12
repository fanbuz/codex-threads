CARGO ?= cargo
CARGO_HOME ?= $(CURDIR)/.cargo-home
INSTALL_BIN ?= $(HOME)/.local/bin
BINARY := target/release/codex-threads

.PHONY: test fmt install-local

test:
	CARGO_HOME="$(CARGO_HOME)" $(CARGO) test

fmt:
	CARGO_HOME="$(CARGO_HOME)" $(CARGO) fmt --all

install-local:
	CARGO_HOME="$(CARGO_HOME)" $(CARGO) build --release
	mkdir -p "$(INSTALL_BIN)"
	cp "$(BINARY)" "$(INSTALL_BIN)/codex-threads"
