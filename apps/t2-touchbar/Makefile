CARGO ?= cargo
INSTALL ?= install
PREFIX ?= /usr/local
DESTDIR ?=

BIN := vort2x-touchbar
BIN_SRC := $(or $(CARGO_TARGET_DIR),target)/release/$(BIN)
BIN_DIR := $(DESTDIR)$(PREFIX)/bin
USER_UNIT_DIR := $(DESTDIR)$(PREFIX)/lib/systemd/user
SYSTEM_UNIT_DIR := $(DESTDIR)$(PREFIX)/lib/systemd/system
UDEV_DIR := $(DESTDIR)$(PREFIX)/lib/udev/rules.d
CONFIG_DIR := $(DESTDIR)/etc/vort2x
LICENSE_DIR := $(DESTDIR)$(PREFIX)/share/licenses/vort2x-touchbar

.PHONY: all build test check-build install uninstall clean
all: build

build:
	$(CARGO) build --release --locked

test:
	$(CARGO) test --locked

check-build:
	@test -x "$(BIN_SRC)" || { echo "error: $(BIN_SRC) is missing; run make build first" >&2; exit 1; }

install: check-build
	$(INSTALL) -d -m 0755 $(BIN_DIR) $(USER_UNIT_DIR) $(SYSTEM_UNIT_DIR) $(UDEV_DIR) $(CONFIG_DIR) $(LICENSE_DIR)
	$(INSTALL) -m 0755 $(BIN_SRC) $(BIN_DIR)/$(BIN)
	$(INSTALL) -m 0644 integration/systemd/user/vort2x-touchbar.service $(USER_UNIT_DIR)/vort2x-touchbar.service
	$(INSTALL) -m 0644 integration/systemd/system/vort2x-touchbar-attach.service $(SYSTEM_UNIT_DIR)/vort2x-touchbar-attach.service
	$(INSTALL) -m 0644 integration/udev/99-zz-vort2x-touchbar.rules $(UDEV_DIR)/99-zz-vort2x-touchbar.rules
	$(INSTALL) -m 0644 THIRD-PARTY-NOTICES.md $(LICENSE_DIR)/THIRD-PARTY-NOTICES.md
	@test -e $(CONFIG_DIR)/touchbar.toml || $(INSTALL) -m 0644 config/touchbar.toml $(CONFIG_DIR)/touchbar.toml
	rm -f $(DESTDIR)$(PREFIX)/lib/modprobe.d/vort2x-touchbar.conf

uninstall:
	rm -f $(BIN_DIR)/$(BIN)
	rm -f $(USER_UNIT_DIR)/vort2x-touchbar.service
	rm -f $(SYSTEM_UNIT_DIR)/vort2x-touchbar-attach.service
	rm -f $(UDEV_DIR)/99-zz-vort2x-touchbar.rules
	rm -f $(DESTDIR)$(PREFIX)/lib/modprobe.d/vort2x-touchbar.conf
	rm -f $(LICENSE_DIR)/THIRD-PARTY-NOTICES.md

clean:
	$(CARGO) clean
