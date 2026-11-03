# pbca: build, test, lint and install, a cheap `smoke`, and the `verify` gate.

BIN := pbca
CARGO := cargo

.PHONY: build release test lint fmt fmt-check check smoke verify install

build: release

release: SIGN_TARGET = target/release/$(BIN)
release:
	$(CARGO) build --release --locked
	@$(SIGN)

test:
	$(CARGO) test --all --locked

lint:
	$(CARGO) clippy --all-targets --locked -- -D warnings

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

check: fmt-check lint test

# No network: version, help and discovery only.
smoke: release
	./target/release/$(BIN) --version
	./target/release/$(BIN) --help > /dev/null
	./target/release/$(BIN) info > /dev/null
	./target/release/$(BIN) mcp --list-tools > /dev/null

# The family's name for the pre-push gate.
verify: fmt-check lint test smoke

# `cargo install` ad-hoc signs, giving the binary a new code identity on every
# install. Re-signing with the family's stable pk-cli-codesign identity keeps
# it on one identity across installs (see cli-common/scripts).
install: SIGN_TARGET = $${CARGO_INSTALL_ROOT:-$$HOME/.cargo}/bin/pbca
install:
	cargo install --path . --locked --force
	@$(SIGN)

# No-ops with a note when the helper or identity is absent (CI, Linux, a
# machine that hasn't run setup-dev-signing.sh).
define SIGN
if [ -x "$$HOME/Dev/cli-common/scripts/dev-sign.sh" ]; then \
	"$$HOME/Dev/cli-common/scripts/dev-sign.sh" "$(SIGN_TARGET)"; \
else echo "cli-common/scripts/dev-sign.sh not found — $(SIGN_TARGET) left ad-hoc signed"; fi
endef
