CARGO ?= cargo
INSTALL ?= install
GTK_UPDATE_ICON_CACHE ?= gtk-update-icon-cache

APP_ID := org.t2powertune.gtk
BIN_NAME := t2-power-tune
BUILD_DIR := $(or $(CARGO_TARGET_DIR),target)/release
BIN_SRC := $(BUILD_DIR)/$(BIN_NAME)

PREFIX ?= /usr/local
BIN_DIR := $(PREFIX)/bin
LIBEXEC_DIR := $(PREFIX)/libexec
APPS_DIR := $(PREFIX)/share/applications
ICONS_DIR := $(PREFIX)/share/icons/hicolor/scalable/apps
ICON_THEME_DIR := $(PREFIX)/share/icons/hicolor
POLICY_DIR ?= /usr/share/polkit-1/actions
SERVICE := /etc/systemd/system/vort2x-power-tune.service

.PHONY: all build test check-build install uninstall clean rebuild

all: build

build:
	$(CARGO) build --release --locked

test:
	$(CARGO) test --locked

check-build:
	@test -x "$(BIN_SRC)" || { echo "error: $(BIN_SRC) is missing; run 'make build' as your normal user first" >&2; exit 1; }

# An existing selection gets the units of this version, including the one
# that reapplies it after resume.
install: check-build
	$(INSTALL) -d -o root -g root -m 0755 $(BIN_DIR) $(LIBEXEC_DIR) $(APPS_DIR) $(ICONS_DIR) $(POLICY_DIR)
	$(INSTALL) -o root -g root -m 0755 $(BIN_SRC) $(BIN_DIR)/$(BIN_NAME)
	$(INSTALL) -o root -g root -m 0755 t2-power-tune-helper $(LIBEXEC_DIR)/t2-power-tune-helper
	$(INSTALL) -o root -g root -m 0755 t2-power-tune-status $(LIBEXEC_DIR)/t2-power-tune-status
	rm -f $(LIBEXEC_DIR)/t2-power-tune-cstates
	@if [ -f $(SERVICE) ]; then $(LIBEXEC_DIR)/t2-power-tune-helper refresh || echo "warning: could not refresh vort2x-power-tune.service, save the selection again in T2 Power Tune" >&2; fi
	$(INSTALL) -o root -g root -m 0644 org.t2powertune.policy $(POLICY_DIR)/org.t2powertune.policy
	$(INSTALL) -o root -g root -m 0644 $(APP_ID).desktop $(APPS_DIR)/$(APP_ID).desktop
	$(INSTALL) -o root -g root -m 0644 $(APP_ID).svg $(ICONS_DIR)/$(APP_ID).svg
	$(GTK_UPDATE_ICON_CACHE) --force --ignore-theme-index $(ICON_THEME_DIR)

uninstall:
	rm -f $(BIN_DIR)/$(BIN_NAME) $(LIBEXEC_DIR)/t2-power-tune-helper $(LIBEXEC_DIR)/t2-power-tune-status
	rm -f $(POLICY_DIR)/org.t2powertune.policy $(APPS_DIR)/$(APP_ID).desktop $(ICONS_DIR)/$(APP_ID).svg
	@if [ -d "$(ICON_THEME_DIR)" ]; then $(GTK_UPDATE_ICON_CACHE) --force --ignore-theme-index $(ICON_THEME_DIR); fi

clean:
	$(CARGO) clean

rebuild: clean build
