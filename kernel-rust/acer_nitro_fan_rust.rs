// SPDX-License-Identifier: GPL-3.0-only
//! Acer Nitro fan-speed WMI driver implemented in Rust.
//!
//! This is a behavior-preserving rewrite of the fan-control portion of DAM's
//! `acer_nitro_gaming_driver2.c`. It deliberately omits keyboard-light control.

use core::{
    ffi::{c_char, c_void},
    sync::atomic::{AtomicBool, Ordering},
};

use kernel::{
    fs::{File, Kiocb},
    iov::IovIterSource,
    miscdevice::{MiscDevice, MiscDeviceOptions, MiscDeviceRegistration},
    prelude::*,
};

module! {
    type: AcerNitroFanModule,
    name: "acer_nitro_fan_rust",
    authors: ["Abhishek Sinha"],
    description: "Acer Nitro WMI fan-speed driver in Rust",
    license: "GPL",
}

const WMI_GUID: &[u8] = b"7A4DDFE7-5B5D-40B4-8595-4408E0CC7F56\0";
const METHOD_UNLOCK: u32 = 14;
const METHOD_SET_FAN: u32 = 16;
const UNLOCK_ONE: u32 = 7_681;
const UNLOCK_TWO: u32 = 1_638_410;
const CPU_FAN_ID: u32 = 1;
const GPU_FAN_ID: u32 = 4;
const FAN_MAX: u32 = 2_560;
const FAN_RESTORE: u32 = 512;
const MAX_WRITE_BYTES: usize = 6; // "2560\r\n"
const ACPI_TYPE_INTEGER: u32 = 1;
const AE_OK: u32 = 0;

/// C layout of the integer member of `union acpi_object` on x86_64.
#[repr(C)]
#[derive(Copy, Clone)]
struct AcpiInteger {
    object_type: u32,
    _padding: u32,
    value: u64,
}

/// `union acpi_object` has size 24 and alignment 8 on the supported x86_64 ABI.
#[repr(C)]
union AcpiObject {
    integer: AcpiInteger,
    _size_and_alignment: [u64; 3],
}

#[repr(C)]
struct AcpiBuffer {
    length: usize,
    pointer: *mut c_void,
}

const _: () = assert!(core::mem::size_of::<AcpiObject>() == 24);
const _: () = assert!(core::mem::align_of::<AcpiObject>() == 8);

extern "C" {
    fn wmi_has_guid(guid: *const c_char) -> bool;
    fn wmi_evaluate_method(
        guid: *const c_char,
        instance: u8,
        method_id: u32,
        input: *const AcpiBuffer,
        output: *mut AcpiBuffer,
    ) -> u32;
}

static WMI_BUSY: AtomicBool = AtomicBool::new(false);

struct WmiGuard;

impl WmiGuard {
    fn acquire() -> Result<Self> {
        WMI_BUSY
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map(|_| Self)
            .map_err(|_| EBUSY)
    }
}

impl Drop for WmiGuard {
    fn drop(&mut self) {
        WMI_BUSY.store(false, Ordering::Release);
    }
}

fn invoke_wmi(method_id: u32, value: u32) -> Result {
    let mut argument = AcpiObject {
        integer: AcpiInteger {
            object_type: ACPI_TYPE_INTEGER,
            _padding: 0,
            value: u64::from(value),
        },
    };
    let input = AcpiBuffer {
        length: core::mem::size_of::<AcpiObject>(),
        pointer: (&mut argument as *mut AcpiObject).cast(),
    };

    // SAFETY: WMI_GUID is NUL-terminated and lives for the entire call. `input`
    // points to a correctly aligned, initialized ACPI integer object. The output
    // pointer is null because these firmware methods have no consumed result.
    let status = unsafe {
        wmi_evaluate_method(
            WMI_GUID.as_ptr().cast(),
            0,
            method_id,
            &input,
            core::ptr::null_mut(),
        )
    };

    if status == AE_OK {
        Ok(())
    } else {
        Err(EIO)
    }
}

fn set_fan_speed(fan_id: u32, speed: u32) -> Result {
    if fan_id != CPU_FAN_ID && fan_id != GPU_FAN_ID {
        return Err(EINVAL);
    }
    if speed > FAN_MAX {
        return Err(ERANGE);
    }

    let encoded = speed
        .checked_mul(10)
        .and_then(|value| value.checked_add(fan_id))
        .ok_or(EOVERFLOW)?;
    invoke_wmi(METHOD_SET_FAN, encoded)
}

fn parse_speed(input: &[u8]) -> Result<u32> {
    let mut end = input.len();
    if end > 0 && input[end - 1] == b'\n' {
        end -= 1;
    }
    if end > 0 && input[end - 1] == b'\r' {
        end -= 1;
    }
    if end == 0 {
        return Err(EINVAL);
    }

    let mut value = 0_u32;
    for &byte in &input[..end] {
        if !byte.is_ascii_digit() {
            return Err(EINVAL);
        }
        value = value
            .checked_mul(10)
            .and_then(|number| number.checked_add(u32::from(byte - b'0')))
            .ok_or(EOVERFLOW)?;
    }

    if value > FAN_MAX {
        Err(ERANGE)
    } else {
        Ok(value)
    }
}

fn write_speed(fan_id: u32, iov: &mut IovIterSource<'_>) -> Result<usize> {
    let length = iov.len();
    if length == 0 {
        return Err(EINVAL);
    }
    if length > MAX_WRITE_BYTES {
        return Err(EMSGSIZE);
    }

    let mut input = [0_u8; MAX_WRITE_BYTES];
    if iov.copy_from_iter(&mut input[..length]) != length {
        return Err(EFAULT);
    }
    let speed = parse_speed(&input[..length])?;

    let _guard = WmiGuard::acquire()?;
    set_fan_speed(fan_id, speed)?;
    Ok(length)
}

struct CpuFanDevice {
    _private: u8,
}

#[vtable]
impl MiscDevice for CpuFanDevice {
    type Ptr = KBox<Self>;

    fn open(_file: &File, _misc: &MiscDeviceRegistration<Self>) -> Result<Self::Ptr> {
        KBox::new(Self { _private: 0 }, GFP_KERNEL)
    }

    fn write_iter(
        _kiocb: Kiocb<'_, Self::Ptr>,
        iov: &mut IovIterSource<'_>,
    ) -> Result<usize> {
        write_speed(CPU_FAN_ID, iov)
    }
}

struct GpuFanDevice {
    _private: u8,
}

#[vtable]
impl MiscDevice for GpuFanDevice {
    type Ptr = KBox<Self>;

    fn open(_file: &File, _misc: &MiscDeviceRegistration<Self>) -> Result<Self::Ptr> {
        KBox::new(Self { _private: 0 }, GFP_KERNEL)
    }

    fn write_iter(
        _kiocb: Kiocb<'_, Self::Ptr>,
        iov: &mut IovIterSource<'_>,
    ) -> Result<usize> {
        write_speed(GPU_FAN_ID, iov)
    }
}

fn initialize_firmware() -> Result {
    // SAFETY: WMI_GUID is a valid, NUL-terminated string with static lifetime.
    if !unsafe { wmi_has_guid(WMI_GUID.as_ptr().cast()) } {
        return Err(ENODEV);
    }

    let _guard = WmiGuard::acquire()?;
    invoke_wmi(METHOD_UNLOCK, UNLOCK_ONE)?;
    invoke_wmi(METHOD_UNLOCK, UNLOCK_TWO)?;
    set_fan_speed(CPU_FAN_ID, FAN_RESTORE)?;
    set_fan_speed(GPU_FAN_ID, FAN_RESTORE)
}

fn restore_firmware_defaults() {
    let _guard = match WmiGuard::acquire() {
        Ok(guard) => guard,
        Err(_) => return,
    };

    if set_fan_speed(CPU_FAN_ID, FAN_RESTORE).is_err() {
        pr_err!("failed to restore CPU fan default\n");
    }
    if set_fan_speed(GPU_FAN_ID, FAN_RESTORE).is_err() {
        pr_err!("failed to restore GPU fan default\n");
    }
}

#[pin_data(PinnedDrop)]
struct AcerNitroFanModule {
    #[pin]
    _cpu: MiscDeviceRegistration<CpuFanDevice>,
    #[pin]
    _gpu: MiscDeviceRegistration<GpuFanDevice>,
}

impl kernel::InPlaceModule for AcerNitroFanModule {
    fn init(_module: &'static ThisModule) -> impl PinInit<Self, Error> {
        try_pin_init!(Self {
            _cpu <- {
                initialize_firmware()?;
                pr_info!("Acer Nitro WMI interface unlocked\n");
                MiscDeviceRegistration::register(MiscDeviceOptions { name: c"fan1" })
            },
            _gpu <- MiscDeviceRegistration::register(MiscDeviceOptions { name: c"fan2" }),
        })
    }
}

#[pinned_drop]
impl PinnedDrop for AcerNitroFanModule {
    fn drop(self: Pin<&mut Self>) {
        restore_firmware_defaults();
        pr_info!("Acer Nitro fan defaults restored\n");
    }
}
