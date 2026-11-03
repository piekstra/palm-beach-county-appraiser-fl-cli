# Convenience targets for pbca.

.PHONY: build test lint fmt fmt-check check verify install

build:
	cargo build --release

test:
	cargo test

lint:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

check: fmt-check lint test

# The family's name for the pre-push gate; the same checks as `check`.
verify: check

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
