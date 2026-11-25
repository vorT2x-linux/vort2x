CARGO ?= cargo
INSTALL ?= install
GTK_UPDATE_ICON_CACHE ?= gtk-update-icon-cache

APP_ID := org.vort2x.Dashboard
BIN_NAME := vort2x-dashboard
BUILD_DIR := $(or $(CARGO_TARGET_DIR),target)/release
BIN_SRC := $(BUILD_DIR)/$(BIN_NAME)

PREFIX ?= /usr/local
BIN_DIR := $(PREFIX)/bin
APPS_DIR := $(PREFIX)/share/applications
ICONS_DIR := $(PREFIX)/share/icons/hicolor/scalable/apps
ICON_THEME_DIR := $(PREFIX)/share/icons/hicolor

.PHONY: all build test check-build install uninstall clean rebuild

all: build

build:
	$(CARGO) build --release --locked

test:
	$(CARGO) test --locked

check-build:
	@test -x "$(BIN_SRC)" || { echo "error: $(BIN_SRC) is missing; run 'make build' as your normal user first" >&2; exit 1; }

install: check-build
	$(INSTALL) -d -o root -g root -m 0755 $(BIN_DIR) $(APPS_DIR) $(ICONS_DIR)
	$(INSTALL) -o root -g root -m 0755 $(BIN_SRC) $(BIN_DIR)/$(BIN_NAME)
	$(INSTALL) -o root -g root -m 0644 $(APP_ID).desktop $(APPS_DIR)/$(APP_ID).desktop
	$(INSTALL) -o root -g root -m 0644 icons/hicolor/scalable/apps/$(APP_ID).svg $(ICONS_DIR)/$(APP_ID).svg
	$(GTK_UPDATE_ICON_CACHE) --force --ignore-theme-index $(ICON_THEME_DIR)

uninstall:
	rm -f $(BIN_DIR)/$(BIN_NAME) $(APPS_DIR)/$(APP_ID).desktop $(ICONS_DIR)/$(APP_ID).svg
	@if [ -d "$(ICON_THEME_DIR)" ]; then $(GTK_UPDATE_ICON_CACHE) --force --ignore-theme-index $(ICON_THEME_DIR); fi

clean:
	$(CARGO) clean

rebuild: clean build
