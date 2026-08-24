# Acer Nitro fan kernel driver in Rust

This directory contains a Rust-for-Linux rewrite of only the fan-control part
of DAM's Acer Nitro kernel module. It creates `/dev/fan1` for the CPU fan and
`/dev/fan2` for the GPU fan. Each device accepts one decimal speed from `0` to
`2560`, optionally followed by CR/LF.

## Safety changes from DAM

- Rejects writes larger than six bytes and values outside `0..=2560`.
- Uses a fixed stack buffer; the write path performs no heap allocation.
- Uses checked arithmetic when encoding the firmware argument.
- Serializes WMI calls and returns `EBUSY` instead of racing firmware calls.
- Propagates WMI/ACPI failures to userspace as `EIO`.
- Registers device nodes only after GUID detection and successful unlock.
- Restores speed `512` for both fans when the module unloads.
- Does not alter keyboard lighting.
- Leaves device permissions to udev instead of forcing mode `0666`.

## Kernel requirement

This is an external Rust-for-Linux module, not a Cargo program. Cargo metadata
is intentionally absent. Kbuild compiles it through the kernel's pinned Rust
toolchain and `kernel` crate.

The kernel must have `CONFIG_RUST=y`, and `KDIR` must point to a fully prepared
matching kernel build tree containing `rust/kernel/lib.rs` and generated Rust
metadata. Ubuntu's ordinary header package on this machine enables Rust but
does not ship those files, so `/lib/modules/$(uname -r)/build` is currently
insufficient.

```sh
make LLVM=1 rustavailable
make LLVM=1 prepare modules_prepare
make -C /path/to/prepared/linux M="$PWD" LLVM=1
```

Do not load this module while DAM's `acer_nitro_gaming_driver2` is loaded:

```sh
sudo modprobe -r acer_nitro_gaming_driver2
sudo insmod acer_nitro_fan_rust.ko
```

The source currently targets the x86_64 ACPI ABI and the contemporary
Rust-for-Linux miscdevice API. It has not been compiled on this machine because
the required kernel Rust build artifacts are absent. Treat it as development
code until it has been built, reviewed, and tested on supported hardware.

## Licensing

This rewrite was produced after inspecting GPLv3 DAM source, so this directory
is kept separate from AFC's MIT userspace code and is licensed GPL-3.0-only.
The module metadata uses Linux's conventional `GPL` declaration so it may call
GPL-only WMI exports. Obtain appropriate legal review before redistribution or
upstream submission.
