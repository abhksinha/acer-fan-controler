# Acer Fan Controler (AFC)

AFC is a minimal, freestanding Rust fan-control daemon for selected Acer Nitro
and Predator laptops running Linux. It reads trusted CPU/GPU temperatures
through Linux `hwmon` and controls fan endpoints provided by the
[DAM Fan Controls](https://github.com/PXDiv/Div-Acer-Manager-Fan-Controls)
WMI kernel driver.

> [!WARNING]
> Fan control is hardware-specific. Incorrect values can cause inadequate
> cooling, fan wear, thermal throttling, shutdown, firmware instability, or
> hardware damage. AFC refuses unknown ranges instead of probing or guessing
> them. Verify the range for the exact laptop before enabling AFC.

## Status and compatibility

AFC is experimental systems software.

| Area | Status |
| --- | --- |
| Acer Nitro V 15 ANV15-51 | Verified locally |
| `512-2560`, step `128` | Verified locally |
| DAM `/dev/fan1` and `/dev/fan2` | Supported |
| Intel `coretemp` | Verified locally |
| AMD `k10temp` and `amdgpu` | Implemented; hardware testing needed |
| NVIDIA Linux `hwmon` | Implemented; hardware testing needed |
| `128-512`, step `128` | Implemented; hardware testing needed |
| Non-x86-64 systems | Not supported |
| Acer systems without DAM gaming WMI | Not supported |

Only ANV15-51 has a built-in range. Other models require explicit
configuration. Similar branding does not prove identical firmware, protocol,
or fan values.

## Features

- Freestanding `no_std` and `no_main` Rust
- No heap, libc, CRT, dynamic loader, Cargo runtime, or external crate
- One process, one thread, fixed buffers, and bounded discovery
- Static x86-64 binary of approximately 10 KiB
- Acer DMI, DAM WMI GUID, and module validation
- Character-device validation for both fan endpoints
- Dynamic `hwmonN` discovery instead of a fixed path
- Hottest trusted CPU/GPU sensor drives the fan target
- Immediate speed increase and delayed gradual decrease
- Maximum configured speed after runtime sensor failure
- Maximum-speed attempt followed by failure after fan-write errors
- Driver-default restoration on controlled shutdown
- Minimal journal reporting every five minutes
- Machine-readable `/run/afc/status`
- Destination compilation and atomic systemd installation
- Hardened service, conflict handling, and activation rollback

## Architecture

```text
CPU/GPU hwmon sensors
          |
          v
  AFC temperature policy
          |
          v
 /dev/fan1 + /dev/fan2
          |
          v
 DAM Acer WMI kernel driver
          |
          v
 Acer firmware / embedded controller
```

AFC is a userspace controller. It does not implement Acer WMI or replace the
kernel driver. DAM accepts ASCII decimal fan values, so AFC uses ASCII at that
required boundary and fixed-width integers internally.

## DAM relationship and attribution

AFC is independent and is not affiliated with Acer or DAM. It depends on the
driver distributed by:

- [PXDiv/Div-Acer-Manager-Fan-Controls](https://github.com/PXDiv/Div-Acer-Manager-Fan-Controls)
- Original work credited by DAM to
  [DetuxTR/AcerNitroLinuxGamingDriver](https://github.com/DetuxTR/AcerNitroLinuxGamingDriver)

DAM documents two possible ranges, both in multiples of `128`:

- `512-2560`
- `128-512`

AFC never discovers a range by writing experimental values. The DAM driver is
not bundled here; install it separately and follow its license requirements.

## Requirements

### Hardware

- Acer laptop
- DAM-compatible WMI GUID:

```text
7A4DDFE7-5B5D-40B4-8595-4408E0CC7F56
```

- At least one DAM character device: `/dev/fan1` or `/dev/fan2`

### Software

- x86-64 Linux
- systemd and Linux `hwmon`
- Loaded or loadable `acer_nitro_gaming_driver2`
- `bash`, `rustc`, `cc`, `readelf`, `systemctl`, and `flock`
- Standard GNU/Linux core utilities

Compilation happens on the destination laptop. No prebuilt binary is required.

## Files

| File | Purpose |
| --- | --- |
| `afc.rs` | Freestanding Rust controller |
| `install-afc.sh` | Destination compiler, installer, and activator |
| `afc-progress.xml` | Implementation and verification status |
| `README.md` | Project documentation |

The local `afc` executable is a build artifact and need not be committed.

## Pre-installation checks

Check identity:

```bash
cat /sys/class/dmi/id/sys_vendor
cat /sys/class/dmi/id/product_name
```

Check WMI:

```bash
test -e /sys/bus/wmi/devices/7A4DDFE7-5B5D-40B4-8595-4408E0CC7F56 \
  && echo "DAM WMI interface present"
```

Check the module and fan devices:

```bash
test -d /sys/module/acer_nitro_gaming_driver2 && echo "module loaded"
stat /dev/fan1 /dev/fan2
```

Fan endpoints must be character devices. AFC rejects ordinary files with those
names.

List sensor providers:

```bash
find /sys/class/hwmon -mindepth 2 -maxdepth 2 -name name -type f \
  -exec sh -c 'printf "%s: " "$1"; cat "$1"' sh '{}' \;
```

AFC currently trusts `coretemp`, `k10temp`, `amdgpu`, and `nvidia`.

## Installation

Clone or download the repository on the target laptop:

```bash
chmod 0755 install-afc.sh
sudo ./install-afc.sh
```

ANV15-51 uses the verified built-in range:

```text
512:2560:128:512
```

The fields are `MIN:MAX:STEP:RESTORE`.

For another DAM-compatible model, provide a verified range:

```bash
sudo ./install-afc.sh --range 512:2560:128:512
```

For hardware verified to use the smaller range:

```bash
sudo ./install-afc.sh --range 128:512:128:512
```

Install and enable without immediate startup:

```bash
sudo ./install-afc.sh --no-start
```

Do not copy a range from an unrelated model based on its name or fan sound.

## What the installer does

1. Requires root and x86-64.
2. Validates Acer DMI and DAM gaming WMI.
3. Loads `acer_nitro_gaming_driver2` with `modprobe` when needed.
4. Waits for `udev` and validates fan character devices.
5. Validates existing or command-line range configuration.
6. Compiles in a private staging directory with environment Rust flags removed.
7. Rejects output containing a dynamic ELF interpreter.
8. Installs source, binary, configuration, and unit atomically.
9. Starts AFC and verifies that systemd reports it active.
10. Disables known conflicts only after successful activation.
11. Restores previous AFC artifacts and service state after activation failure.

Installed paths:

```text
/usr/local/src/afc/afc.rs
/usr/local/sbin/afc
/etc/systemd/system/afc.service
/etc/afc.conf                 # explicit range only
/run/afc/status               # runtime
```

Concurrent runs use `/run/lock/afc-install.lock`.

## Configuration

Unknown models require `/etc/afc.conf`:

```ini
fan_min=512
fan_max=2560
fan_step=128
fan_restore=512
```

All fields are mandatory when the file exists.

- `fan_min >= 128`
- `fan_max <= 2560`
- `fan_min < fan_max`
- `fan_step > 0`
- Minimum and maximum must be divisible by the step
- Restore must be inside the range

The current DAM driver defines `512` as its default. Confirm this assumption
for the exact driver and laptop. After changing configuration, rerun the
installer so it validates the file.

## Temperature policy

AFC scans `hwmon` at startup, retains at most eight trusted inputs, polls every
two seconds, and uses the hottest valid reading.

For the verified `512-2560` range:

| Temperature | Fan value |
| --- | ---: |
| Below `45 C` | `1408` |
| `45-54 C` | `1536` |
| `55-64 C` | `1792` |
| `65-74 C` | `2176` |
| `75-79 C` | `2432` |
| `80 C` and above | `2560` |

Other valid ranges use the same curve fractions rounded to their nearest step.

### Increasing temperature

A higher target is applied immediately.

### Decreasing temperature

1. AFC waits 20 seconds after entering a lower range.
2. It decreases by one configured step.
3. It continues one step per two-second cycle.
4. It stops at the current target.

This avoids abrupt reductions and reduces boundary oscillation.

### Fail-safe behavior

- No trusted sensor at startup: AFC refuses to run.
- All sensors fail at runtime: AFC requests maximum speed.
- Fan write fails: AFC attempts maximum speed and exits with failure.
- `SIGINT` or `SIGTERM`: AFC writes `fan_restore` and closes descriptors.

`SIGKILL`, power loss, kernel panic, and firmware failure cannot run userspace
cleanup. Safety must not depend solely on AFC's shutdown handler.

## Service operation

```bash
systemctl status afc.service
journalctl -u afc.service -f
cat /run/afc/status
```

Example:

```text
temp=72C speed=2176 sensors=1
```

Manage the service:

```bash
sudo systemctl restart afc.service
sudo systemctl stop afc.service
sudo systemctl disable --now afc.service
```

The journal receives a line initially and about every five minutes. The status
file updates each control cycle.

## Competing controllers

Only one process should control DAM fan devices. AFC conflicts with:

```text
fanctl-rust.service
fanctl.service
dam-fan-controls.service
dam-fan-controls-daemon.service
```

The installer does not delete them. After AFC activates, previously enabled
known conflicts are disabled to prevent boot-time races.

## systemd hardening

The generated unit applies:

- No new privileges and no retained capabilities
- Device access restricted to detected fan endpoints
- Read-only system paths with only `/run/afc` writable
- Protected home, hostname, clock, kernel, logs, and cgroups
- Restricted process visibility and namespaces
- No realtime or set-user-ID transitions
- Locked execution personality
- Writable-and-executable memory prohibited
- Native syscall architecture and dangerous syscall groups denied
- Private temporary directory and bounded restart rate

The daemon cannot compile or insert modules. DAM/DKMS owns module installation;
the installer may use `modprobe` before AFC enters its sandbox.

## Build design

AFC uses:

```rust
#![no_std]
#![no_main]
```

It has no Cargo dependencies. It supplies only the x86-64 runtime boundary:

- `_start` aligns the stack before entering Rust.
- A signal restorer invokes `rt_sigreturn`.
- One inline-assembly function implements the Linux syscall ABI.
- Small `memcpy` and `bcmp` functions satisfy compiler-generated operations.

Control logic remains ordinary Rust. Assembly is not used where safe Rust can
express the operation.

Build settings:

```text
--edition=2021
-C opt-level=z
-C panic=abort
-C lto=fat
-C codegen-units=1
-C overflow-checks=yes
-C strip=symbols
-C relocation-model=static
-nostartfiles
-static
-no-pie
```

`target-cpu=native` is omitted for x86-64 portability.

## Resource model

- Constant bounded memory and no heap
- No worker threads, networking, or IPC listener
- At most eight sensor descriptors and two fan descriptors
- One optional status descriptor
- One two-second cycle

The loop is `O(S)` where `S <= 8`, so work is strictly bounded.

## Manual development build

```bash
rustc afc.rs \
  --edition=2021 \
  -C opt-level=z \
  -C panic=abort \
  -C lto=fat \
  -C codegen-units=1 \
  -C overflow-checks=yes \
  -C strip=symbols \
  -C relocation-model=static \
  -C link-arg=-nostartfiles \
  -C link-arg=-static \
  -C link-arg=-no-pie \
  -C link-arg=-Wl,--build-id=none \
  -o afc
```

Inspect without execution:

```bash
file afc
readelf -hW afc
readelf -dW afc
```

Never run a development build while another controller is active.

## Troubleshooting

### `unsupported non-Acer system`

DMI does not identify Acer. AFC will not bypass this check.

### `DAM WMI interface not found`

Check `/sys/bus/wmi/devices/` for the required GUID.

### `DAM fan module is not loaded`

```bash
sudo modprobe acer_nitro_gaming_driver2
```

If that fails, install the DAM driver for the running kernel.

### `unknown fan range`

Find trustworthy model-specific information and reinstall:

```bash
sudo ./install-afc.sh --range MIN:MAX:STEP:RESTORE
```

### `no valid fan character devices`

```bash
stat /dev/fan1 /dev/fan2
```

They must be character devices. Never replace them with ordinary files.

### `no trusted CPU/GPU temperature sensor`

List providers using the pre-installation command. New providers require review
and hardware testing. Do not substitute unrelated NVMe, Wi-Fi, battery, or
ACPI-zone temperatures without validation.

### Restart loop

```bash
systemctl status afc.service
journalctl -u afc.service -n 100 --no-pager
sudo systemctl reset-failed afc.service
sudo systemctl restart afc.service
```

## Uninstallation

```bash
sudo systemctl disable --now afc.service
sudo rm -f /etc/systemd/system/afc.service
sudo rm -f /usr/local/sbin/afc
sudo rm -f /usr/local/src/afc/afc.rs
sudo systemctl daemon-reload
```

`/etc/afc.conf` is preserved. Remove it only when no longer needed:

```bash
sudo rm -f /etc/afc.conf
```

Re-enable a fallback controller explicitly:

```bash
sudo systemctl enable --now fanctl-rust.service
```

## Contributing

New hardware reports should include:

- DMI product name
- CPU/GPU model, distribution, and kernel
- DAM driver version or revision
- Verified minimum, maximum, step, and restore values
- Available `hwmon` names and labels
- Existing fan devices
- Sanitized AFC logs
- Evidence that maximum and restore behavior work

Do not publish serials, UUIDs, account names, unreviewed logs, or ranges inferred
only from sound.

Performance changes require measured evidence. Unsafe, FFI, syscall, and
assembly changes require explicit safety invariants and architecture testing.

## Roadmap

- Build a reviewed DMI range database from verified reports
- Consume driver-reported capabilities when DAM exposes them
- Add tested sensor providers and label selection
- Package the DAM module through DKMS outside the daemon
- Add ABI, syscall, fault-injection, and installation tests
- Add architectures only with dedicated modules and hardware testing

## License

AFC is licensed under the MIT License. See [LICENSE](LICENSE).

The DAM kernel driver is a separate external project distributed under its own
license. AFC's MIT license does not relicense or replace DAM's license terms.

## Disclaimer

This project is provided without warranty. Firmware and fan behavior vary by
model and revision. Keep firmware thermal protections enabled, monitor
temperatures during initial testing, and retain a known-working recovery path.
