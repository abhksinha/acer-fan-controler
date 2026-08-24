obj-m := acer_nitro_fan_rust.o

KDIR ?= /lib/modules/$(shell uname -r)/build

.PHONY: all clean

all:
	@test -f "$(KDIR)/rust/kernel/lib.rs" || { \
		echo "error: KDIR must be a prepared full Rust-enabled kernel tree" >&2; \
		exit 1; \
	}
	$(MAKE) -C "$(KDIR)" M="$(CURDIR)" modules

clean:
	$(MAKE) -C "$(KDIR)" M="$(CURDIR)" clean
