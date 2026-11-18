CARGO ?= cargo
CARGO_FLAGS ?= --locked
PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
SYSTEMD_UNIT_DIR ?= $(PREFIX)/lib/systemd/system
DATADIR ?= $(PREFIX)/share
SYSCONFDIR ?= /etc
TARGET_DIR ?= $(or $(CARGO_TARGET_DIR),target)

.PHONY: all build test install install-bin install-integration clean
all: build
build:
	$(CARGO) build --release $(CARGO_FLAGS) --target-dir $(TARGET_DIR)
test:
	$(CARGO) test $(CARGO_FLAGS) --target-dir $(TARGET_DIR)
install: install-bin install-integration
install-bin:
	install -D -m 0755 $(TARGET_DIR)/release/t2-touchid $(DESTDIR)$(BINDIR)/t2-touchid
install-integration:
	install -d $(DESTDIR)$(SYSTEMD_UNIT_DIR)
	sed 's|@BINDIR@|$(BINDIR)|g' integration/systemd/vort2x-t2-touchid.service >$(DESTDIR)$(SYSTEMD_UNIT_DIR)/vort2x-t2-touchid.service
	chmod 0644 $(DESTDIR)$(SYSTEMD_UNIT_DIR)/vort2x-t2-touchid.service
	install -D -m 0644 integration/fprintd/fprintd-vort2x-t2-touchid.conf $(DESTDIR)$(SYSTEMD_UNIT_DIR)/fprintd.service.d/vort2x-t2-touchid.conf
	install -D -m 0644 integration/dbus/org.vort2x.TouchId.conf $(DESTDIR)$(DATADIR)/dbus-1/system.d/org.vort2x.TouchId.conf
	install -d $(DESTDIR)$(SYSCONFDIR)/vort2x
	test -e $(DESTDIR)$(SYSCONFDIR)/vort2x/t2-touchid.conf || install -m 0644 config/t2-touchid.conf $(DESTDIR)$(SYSCONFDIR)/vort2x/t2-touchid.conf
clean:
	$(CARGO) clean --target-dir $(TARGET_DIR)
