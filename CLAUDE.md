# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

AFC (Acer Fan Controler) is a fan-control daemon for Acer Nitro/Predator laptops on Linux. Two independent pieces live here, with different licenses and build systems:

- **`afc.rs`** (MIT) — freestanding `no_std`/`no_main` userspace daemon. Single file, no Cargo, no crates, no libc. Writes ASCII decimal speeds to `/dev/fan1` and `/dev/fan2`, which are provided by the external DAM kernel module (`acer_nitro_gaming_driver2`).
- **`kernel-rust/`** (GPL-3.0-only) — Rust-for-Linux rewrite of only the fan part of DAM's module. Creates the same `/dev/fan1`/`/dev/fan2` nodes. Kept in a separate directory on purpose because it was written after reading GPLv3 DAM source; do not move code between the two trees or share files across the license boundary.

There is no Cargo workspace, no test suite, and no CI. `coding_practices.md` and `.enola/` are gitignored local files, not part of the repo.

## Build and verify

### Userspace daemon (`afc.rs`)

Development build (output `./afc` is a build artifact, gitignored):

```bash
rustc afc.rs --edition=2021 -C opt-level=z -C panic=abort -C lto=fat \
  -C codegen-units=1 -C overflow-checks=yes -C strip=symbols \
  -C relocation-model=static -C link-arg=-nostartfiles -C link-arg=-static \
  -C link-arg=-no-pie -C link-arg=-Wl,--build-id=none -o afc
```

Static inspection (the installer enforces the same checks):

```bash
readelf -hW afc          # must be X86-64
readelf -lW afc | grep INTERP   # must print nothing (no dynamic interpreter)
```

Never run a dev build of `afc` while `afc.service` or another fan controller is active. It writes to real fan devices.

Install/activate on the target laptop (compiles on the destination, atomic install, hardened systemd unit, rollback on failure):

```bash
sudo ./install-afc.sh                              # ANV15-51 built-in range
sudo ./install-afc.sh --range MIN:MAX:STEP:RESTORE # other DAM models
sudo ./install-afc.sh --no-start
```

Runtime checks: `systemctl status afc.service`, `journalctl -u afc.service -f`, `cat /run/afc/status` (format `temp=72C speed=2176 sensors=1`).

Flags in `install-afc.sh` and the README "Manual development build" section must stay identical; the installer strips `RUSTFLAGS`/`RUSTC_WRAPPER` from the environment. `target-cpu=native` is deliberately omitted.

### Kernel module (`kernel-rust/`)

Kbuild only; the `Makefile` refuses to run unless `KDIR` points to a fully prepared Rust-enabled kernel tree containing `rust/kernel/lib.rs`. Distro header packages are not enough.

```bash
make -C kernel-rust KDIR=/path/to/prepared/linux LLVM=1
sudo modprobe -r acer_nitro_gaming_driver2   # never load both drivers at once
sudo insmod kernel-rust/acer_nitro_fan_rust.ko
```

As of the last update it has never been compiled on this machine (see `kernel-rust/progress.xml`, verification status `blocked`). Treat API usage against the `kernel` crate (`miscdevice`, `Kiocb`, `IovIterSource`) as unverified.

## Architecture of `afc.rs`

Everything is in one file; read top to bottom.

1. **Runtime boundary** — `global_asm!` `_start` aligns the stack and calls `afc_main`; `afc_sigreturn` is the `SA_RESTORER` for signal handlers; `syscall6` is the single inline-asm syscall wrapper; hand-written `memcpy`/`bcmp` satisfy compiler-emitted calls. Assembly stays confined to these; control logic is ordinary Rust.
2. **Thin syscall helpers** — `open`, `read_once`, `char_device` (via `statx`), `monotonic_sec`, `sleep_poll`. All I/O uses `O_CLOEXEC | O_NOFOLLOW`; fan endpoints are rejected unless `S_IFCHR`.
3. **Compatibility gate** (start of `afc_main`) — DMI vendor contains `Acer`, DAM WMI GUID directory exists under `/sys/bus/wmi/devices/`, module directory exists under `/sys/module/`. Every failure returns a distinct `afc: ...` message on stderr; the README troubleshooting section is keyed on those exact strings, so keep them in sync.
4. **Range resolution** (`detect_range`) — `/etc/afc.conf` wins if present (all four keys mandatory, validated by `valid_range`); otherwise only the `ANV15-51` product string has a built-in range. Unknown models fail. Ranges are never probed by writing to hardware. The bash `validate_range` in the installer mirrors `valid_range` exactly.
5. **Sensor discovery** (`discover_sensors` / `scan_chip`) — walks `/sys/class/hwmon/hwmonN` with raw `getdents64`, keeps only chips whose `name` is in `trusted_hwmon` (`coretemp`, `k10temp`, `amdgpu`, `nvidia`), opens up to `MAX_SENSORS = 8` `tempN_input` files. Fixed-size `Path` buffers (`PATH_CAP = 160`), no heap anywhere.
6. **Control loop** — every `POLL_SEC = 2`: hottest valid reading drives `curve()` (sixteenths of the range, rounded to the nearest step, so the same curve shape applies to any configured range). Rise is immediate; fall waits `DOWN_DELAY_SEC = 20` then drops one step per cycle. Sensor loss drives `range.max`. Fan write failure attempts max then exits 1. `SIGINT`/`SIGTERM` set the `STOP` atomic; loop exits, writes `range.restore`, closes fds.
7. **Reporting** — `publish` pwrites `/run/afc/status` each cycle and echoes to stdout (journal) on first cycle and every `REPORT_SEC = 300`.

Panic handler writes a fixed message and `exit_group(70)`.

## Architecture of `kernel-rust/acer_nitro_fan_rust.rs`

- Two `MiscDevice` impls (`fan1` = CPU id 1, `fan2` = GPU id 4) share `write_speed`: bounded 6-byte read from `IovIterSource`, `parse_speed` (digits, optional CR/LF, `0..=2560`), then `set_fan_speed` encodes `speed * 10 + fan_id` with checked arithmetic and calls `wmi_evaluate_method` method 16.
- `WmiGuard` is a `compare_exchange` spin-free lock on `WMI_BUSY`; concurrent callers get `EBUSY` rather than racing firmware.
- `AcpiObject`/`AcpiBuffer` are hand-laid-out x86_64 ACPI structs with `const` size/align asserts; the WMI exports are declared via `extern "C"`, not bindgen.
- Module init: `wmi_has_guid` check, two unlock calls (method 14 with `7681` and `1638410`), set both fans to 512, then register devices. `PinnedDrop` restores 512 on unload.
- Intentional divergences from DAM: no keyboard lighting, no forced `0666` permissions (udev decides), `EIO`/`EBUSY`/`ERANGE`/`EMSGSIZE` propagated to userspace.
- `UPSTREAM.md` pins the DAM commit and SHA-256 of inspected files; update it if more upstream files are consulted.

## Project conventions

- Both `afc-progress.xml` and `kernel-rust/progress.xml` track component status. Update the relevant one when a component's state changes (e.g. hardware verification done, new sensor provider added).
- Hardware values (fan ranges, trusted hwmon names, unlock constants) are only added with verified evidence from the exact model, never inferred. Adding a model to the built-in range table in `detect_range` requires a verified report per the README "Contributing" section.
- Every `unsafe` block carries a `// SAFETY:` comment; `#![deny(unsafe_op_in_unsafe_fn)]` is on in `afc.rs`.
- `afc.rs` must remain `no_std`, heap-free, single-threaded, and buildable with plain `rustc` using the flags above. Do not introduce Cargo, crates, or libc.
- README is the user-facing spec. Changes to behavior (curve, delays, error strings, installer paths, systemd hardening) must be reflected there.
