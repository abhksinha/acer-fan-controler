// SPDX-License-Identifier: MIT
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

const POLL_SEC: i64 = 2;
const DOWN_DELAY_SEC: i64 = 20;
const REPORT_SEC: i64 = 300;
const MAX_SENSORS: usize = 8;
const PATH_CAP: usize = 160;

const SYS_READ: usize = 0;
const SYS_WRITE: usize = 1;
const SYS_CLOSE: usize = 3;
const SYS_RT_SIGACTION: usize = 13;
const SYS_PREAD64: usize = 17;
const SYS_PWRITE64: usize = 18;
const SYS_NANOSLEEP: usize = 35;
const SYS_FTRUNCATE: usize = 77;
const SYS_CLOCK_GETTIME: usize = 228;
const SYS_EXIT_GROUP: usize = 231;
const SYS_GETDENTS64: usize = 217;
const SYS_OPENAT: usize = 257;
const SYS_STATX: usize = 332;

const AT_FDCWD: isize = -100;
const AT_SYMLINK_NOFOLLOW: usize = 0x100;
const O_WRONLY: usize = 1;
const O_CREAT: usize = 0o100;
const O_TRUNC: usize = 0o1000;
const O_DIRECTORY: usize = 0o200000;
const O_NOFOLLOW: usize = 0o400000;
const O_CLOEXEC: usize = 0o2000000;
const STATX_TYPE: usize = 1;
const S_IFMT: u16 = 0o170000;
const S_IFCHR: u16 = 0o020000;
const CLOCK_MONOTONIC: usize = 1;
const SA_RESTORER: usize = 0x04000000;

const VENDOR_PATH: &[u8] = b"/sys/class/dmi/id/sys_vendor\0";
const PRODUCT_PATH: &[u8] = b"/sys/class/dmi/id/product_name\0";
const WMI_PATH: &[u8] =
    b"/sys/bus/wmi/devices/7A4DDFE7-5B5D-40B4-8595-4408E0CC7F56\0";
const MODULE_PATH: &[u8] = b"/sys/module/acer_nitro_gaming_driver2\0";
const HWMON_PATH: &[u8] = b"/sys/class/hwmon\0";
const FAN1_PATH: &[u8] = b"/dev/fan1\0";
const FAN2_PATH: &[u8] = b"/dev/fan2\0";
const CONFIG_PATH: &[u8] = b"/etc/afc.conf\0";
const STATUS_PATH: &[u8] = b"/run/afc/status\0";

static STOP: AtomicBool = AtomicBool::new(false);

// Freestanding optimized slice operations may lower to these libc ABI symbols.
#[no_mangle]
unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    for offset in 0..len {
        // SAFETY: The caller guarantees non-overlapping readable/writable regions of len bytes.
        let byte = unsafe { core::ptr::read_volatile(src.add(offset)) };
        // SAFETY: The same caller contract guarantees dst.add(offset) is writable.
        unsafe { core::ptr::write_volatile(dst.add(offset), byte) };
    }
    dst
}

#[no_mangle]
unsafe extern "C" fn bcmp(left: *const u8, right: *const u8, len: usize) -> i32 {
    for offset in 0..len {
        // SAFETY: The caller guarantees both regions are readable for len bytes.
        let a = unsafe { core::ptr::read_volatile(left.add(offset)) };
        // SAFETY: The caller guarantees both regions are readable for len bytes.
        let b = unsafe { core::ptr::read_volatile(right.add(offset)) };
        if a != b {
            return 1;
        }
    }
    0
}

global_asm!(
    ".section .text._start,\"ax\",@progbits",
    ".global _start",
    ".type _start,@function",
    "_start:",
    "xor rbp, rbp",
    "and rsp, -16",
    "call afc_main",
    "mov edi, eax",
    "mov eax, 231",
    "syscall",
    "ud2",
    ".size _start, .-_start",
    ".global afc_sigreturn",
    ".type afc_sigreturn,@function",
    "afc_sigreturn:",
    "mov eax, 15",
    "syscall",
    "ud2",
);

extern "C" {
    fn afc_sigreturn();
}

#[derive(Clone, Copy)]
struct Path {
    bytes: [u8; PATH_CAP],
    len: usize,
}

impl Path {
    const EMPTY: Self = Self {
        bytes: [0; PATH_CAP],
        len: 0,
    };

    fn push(&mut self, src: &[u8]) -> bool {
        if self.len + src.len() >= PATH_CAP {
            return false;
        }
        self.bytes[self.len..self.len + src.len()].copy_from_slice(src);
        self.len += src.len();
        self.bytes[self.len] = 0;
        true
    }

    fn c_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }
}

#[derive(Clone, Copy)]
struct Sensor {
    fd: i32,
}

#[derive(Clone, Copy)]
struct FanRange {
    min: u32,
    max: u32,
    step: u32,
    restore: u32,
}

#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}

#[repr(C)]
struct KernelSigAction {
    handler: usize,
    flags: usize,
    restorer: usize,
    mask: u64,
}

#[inline(always)]
unsafe fn syscall6(
    nr: usize,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: usize,
) -> isize {
    let ret: isize;
    // SAFETY: Registers follow the Linux x86-64 syscall ABI; rcx/r11 are clobbered.
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr as isize => ret,
            in("rdi") a,
            in("rsi") b,
            in("rdx") c,
            in("r10") d,
            in("r8") e,
            in("r9") f,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn call(nr: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    // SAFETY: syscall6 contains the architecture-specific ABI boundary.
    unsafe { syscall6(nr, a, b, c, d, e, f) }
}

fn open(path: *const u8, flags: usize, mode: usize) -> i32 {
    call(
        SYS_OPENAT,
        AT_FDCWD as usize,
        path as usize,
        flags,
        mode,
        0,
        0,
    ) as i32
}

fn close(fd: i32) {
    if fd >= 0 {
        let _ = call(SYS_CLOSE, fd as usize, 0, 0, 0, 0, 0);
    }
}

fn read_once(path: *const u8, out: &mut [u8]) -> Option<usize> {
    let fd = open(path, O_CLOEXEC | O_NOFOLLOW, 0);
    if fd < 0 {
        return None;
    }
    let n = call(
        SYS_READ,
        fd as usize,
        out.as_mut_ptr() as usize,
        out.len(),
        0,
        0,
        0,
    );
    close(fd);
    if n <= 0 {
        None
    } else {
        Some(trim_len(&out[..n as usize]))
    }
}

fn path_exists(path: *const u8) -> bool {
    let fd = open(path, O_CLOEXEC | O_DIRECTORY, 0);
    if fd < 0 {
        false
    } else {
        close(fd);
        true
    }
}

fn trim_len(buf: &[u8]) -> usize {
    let mut n = buf.len();
    while n > 0 && matches!(buf[n - 1], b' ' | b'\n' | b'\r' | b'\t' | 0) {
        n -= 1;
    }
    n
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|part| part == needle)
}

fn parse_u32(buf: &[u8]) -> Option<u32> {
    if buf.is_empty() {
        return None;
    }
    let mut value = 0u32;
    for &byte in buf {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add((byte - b'0') as u32)?;
    }
    Some(value)
}

fn config_value(buf: &[u8], key: &[u8]) -> Option<u32> {
    for line in buf.split(|&byte| byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if let Some(value) = line.strip_prefix(key) {
            return parse_u32(value);
        }
    }
    None
}

fn valid_range(range: FanRange) -> bool {
    range.step > 0
        && range.min >= 128
        && range.max <= 2560
        && range.min < range.max
        && range.min % range.step == 0
        && range.max % range.step == 0
        && range.restore >= range.min
        && range.restore <= range.max
}

fn detect_range(product: &[u8]) -> Option<FanRange> {
    let mut cfg = [0u8; 512];
    if let Some(n) = read_once(CONFIG_PATH.as_ptr(), &mut cfg) {
        let range = FanRange {
            min: config_value(&cfg[..n], b"fan_min=")?,
            max: config_value(&cfg[..n], b"fan_max=")?,
            step: config_value(&cfg[..n], b"fan_step=")?,
            restore: config_value(&cfg[..n], b"fan_restore=")?,
        };
        return valid_range(range).then_some(range);
    }

    // This is the only model validated locally; unknown models must configure a range.
    if contains(product, b"ANV15-51") {
        Some(FanRange {
            min: 512,
            max: 2560,
            step: 128,
            restore: 512,
        })
    } else {
        None
    }
}

fn char_device(path: *const u8) -> bool {
    let mut statx = [0u8; 256];
    let ret = call(
        SYS_STATX,
        AT_FDCWD as usize,
        path as usize,
        AT_SYMLINK_NOFOLLOW,
        STATX_TYPE,
        statx.as_mut_ptr() as usize,
        0,
    );
    if ret < 0 {
        return false;
    }
    let mode = u16::from_ne_bytes([statx[28], statx[29]]);
    mode & S_IFMT == S_IFCHR
}

fn open_fan(path: &[u8]) -> i32 {
    if !char_device(path.as_ptr()) {
        return -1;
    }
    open(path.as_ptr(), O_WRONLY | O_CLOEXEC | O_NOFOLLOW, 0)
}

fn trusted_hwmon(name: &[u8]) -> bool {
    name == b"coretemp" || name == b"k10temp" || name == b"amdgpu" || name == b"nvidia"
}

fn hwmon_name(name: &[u8]) -> bool {
    name.len() > 5
        && name.starts_with(b"hwmon")
        && name[5..].iter().all(u8::is_ascii_digit)
}

fn add_sensor(path: &Path, sensors: &mut [Sensor; MAX_SENSORS], count: &mut usize) {
    if *count == MAX_SENSORS {
        return;
    }
    let fd = open(path.c_ptr(), O_CLOEXEC | O_NOFOLLOW, 0);
    if fd >= 0 {
        sensors[*count].fd = fd;
        *count += 1;
    }
}

fn scan_chip(
    entry: &[u8],
    sensors: &mut [Sensor; MAX_SENSORS],
    count: &mut usize,
) {
    let mut base = Path::EMPTY;
    if !base.push(b"/sys/class/hwmon/") || !base.push(entry) || !base.push(b"/") {
        return;
    }
    let mut name_path = base;
    if !name_path.push(b"name") {
        return;
    }
    let mut name = [0u8; 32];
    let Some(n) = read_once(name_path.c_ptr(), &mut name) else {
        return;
    };
    if !trusted_hwmon(&name[..n]) {
        return;
    }
    for index in 1..=32u32 {
        let mut path = base;
        if !path.push(b"temp") || !push_u32(&mut path, index) || !path.push(b"_input") {
            break;
        }
        let mut probe = [0u8; 16];
        if read_once(path.c_ptr(), &mut probe).is_some() {
            add_sensor(&path, sensors, count);
            if *count == MAX_SENSORS {
                break;
            }
        }
    }
}

fn discover_sensors(sensors: &mut [Sensor; MAX_SENSORS]) -> usize {
    let dir = open(
        HWMON_PATH.as_ptr(),
        O_CLOEXEC | O_DIRECTORY | O_NOFOLLOW,
        0,
    );
    if dir < 0 {
        return 0;
    }
    let mut count = 0usize;
    let mut buf = [0u8; 2048];
    loop {
        let n = call(
            SYS_GETDENTS64,
            dir as usize,
            buf.as_mut_ptr() as usize,
            buf.len(),
            0,
            0,
            0,
        );
        if n <= 0 {
            break;
        }
        let mut pos = 0usize;
        while pos + 19 <= n as usize {
            let reclen = u16::from_ne_bytes([buf[pos + 16], buf[pos + 17]]) as usize;
            if reclen < 20 || pos + reclen > n as usize {
                count = 0;
                break;
            }
            let raw = &buf[pos + 19..pos + reclen];
            let len = raw.iter().position(|&byte| byte == 0).unwrap_or(raw.len());
            if hwmon_name(&raw[..len]) {
                scan_chip(&raw[..len], sensors, &mut count);
            }
            pos += reclen;
        }
    }
    close(dir);
    count
}

fn push_u32(path: &mut Path, mut value: u32) -> bool {
    let mut tmp = [0u8; 10];
    let mut pos = tmp.len();
    loop {
        pos -= 1;
        tmp[pos] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            return path.push(&tmp[pos..]);
        }
    }
}

fn sensor_temp(fd: i32) -> Option<u32> {
    let mut buf = [0u8; 16];
    let n = call(
        SYS_PREAD64,
        fd as usize,
        buf.as_mut_ptr() as usize,
        buf.len(),
        0,
        0,
        0,
    );
    if n <= 0 {
        return None;
    }
    let milli = parse_u32(&buf[..trim_len(&buf[..n as usize])])?;
    (1_000..=130_000)
        .contains(&milli)
        .then_some((milli + 500) / 1000)
}

fn hottest(sensors: &[Sensor], count: usize) -> Option<u32> {
    let mut hottest = None;
    for sensor in &sensors[..count] {
        if let Some(temp) = sensor_temp(sensor.fd) {
            hottest = Some(hottest.map_or(temp, |old: u32| old.max(temp)));
        }
    }
    hottest
}

fn curve(temp: u32, range: FanRange) -> u32 {
    let sixteenths = match temp {
        0..=44 => 7,
        45..=54 => 8,
        55..=64 => 10,
        65..=74 => 13,
        75..=79 => 15,
        _ => 16,
    };
    let span = range.max - range.min;
    let delta = span * sixteenths / 16;
    let steps = (delta + range.step / 2) / range.step;
    (range.min + steps * range.step).min(range.max)
}

fn decimal(mut value: u32, out: &mut [u8]) -> usize {
    let mut tmp = [0u8; 10];
    let mut pos = tmp.len();
    loop {
        pos -= 1;
        tmp[pos] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            let n = tmp.len() - pos;
            out[..n].copy_from_slice(&tmp[pos..]);
            return n;
        }
    }
}

fn write_speed(fd: i32, speed: u32) -> bool {
    let mut buf = [0u8; 10];
    let n = decimal(speed, &mut buf);
    call(
        SYS_WRITE,
        fd as usize,
        buf.as_ptr() as usize,
        n,
        0,
        0,
        0,
    ) == n as isize
}

fn set_fans(fans: &[i32; 2], speed: u32) -> bool {
    fans.iter().all(|&fd| fd < 0 || write_speed(fd, speed))
}

fn status_line(temp: u32, speed: u32, sensors: usize, out: &mut [u8; 64]) -> usize {
    let mut n = 0usize;
    for part in [b"temp=".as_slice()] {
        out[n..n + part.len()].copy_from_slice(part);
        n += part.len();
    }
    n += decimal(temp, &mut out[n..]);
    out[n..n + 8].copy_from_slice(b"C speed=");
    n += 8;
    n += decimal(speed, &mut out[n..]);
    out[n..n + 9].copy_from_slice(b" sensors=");
    n += 9;
    n += decimal(sensors as u32, &mut out[n..]);
    out[n] = b'\n';
    n + 1
}

fn publish(fd: i32, line: &[u8], journal: bool) {
    if fd >= 0 {
        let n = call(
            SYS_PWRITE64,
            fd as usize,
            line.as_ptr() as usize,
            line.len(),
            0,
            0,
            0,
        );
        if n >= 0 {
            let _ = call(SYS_FTRUNCATE, fd as usize, n as usize, 0, 0, 0, 0);
        }
    }
    if journal {
        let _ = call(
            SYS_WRITE,
            1,
            line.as_ptr() as usize,
            line.len(),
            0,
            0,
            0,
        );
    }
}

fn monotonic_sec() -> i64 {
    let mut now = Timespec { sec: 0, nsec: 0 };
    if call(
        SYS_CLOCK_GETTIME,
        CLOCK_MONOTONIC,
        &mut now as *mut Timespec as usize,
        0,
        0,
        0,
        0,
    ) < 0
    {
        0
    } else {
        now.sec
    }
}

fn sleep_poll() {
    let req = Timespec {
        sec: POLL_SEC,
        nsec: 0,
    };
    let _ = call(
        SYS_NANOSLEEP,
        &req as *const Timespec as usize,
        0,
        0,
        0,
        0,
        0,
    );
}

extern "C" fn stop_signal(_: i32) {
    STOP.store(true, Ordering::Relaxed);
}

fn install_signal(signal: usize) -> bool {
    let action = KernelSigAction {
        handler: stop_signal as usize,
        flags: SA_RESTORER,
        restorer: afc_sigreturn as usize,
        mask: 0,
    };
    call(
        SYS_RT_SIGACTION,
        signal,
        &action as *const KernelSigAction as usize,
        0,
        8,
        0,
        0,
    ) == 0
}

fn fail(message: &[u8]) -> i32 {
    let _ = call(
        SYS_WRITE,
        2,
        message.as_ptr() as usize,
        message.len(),
        0,
        0,
        0,
    );
    1
}

#[no_mangle]
extern "C" fn afc_main() -> i32 {
    let mut vendor = [0u8; 64];
    let Some(vendor_len) = read_once(VENDOR_PATH.as_ptr(), &mut vendor) else {
        return fail(b"afc: cannot read system vendor\n");
    };
    if !contains(&vendor[..vendor_len], b"Acer") {
        return fail(b"afc: unsupported non-Acer system\n");
    }
    if !path_exists(WMI_PATH.as_ptr()) {
        return fail(b"afc: DAM WMI interface not found\n");
    }
    if !path_exists(MODULE_PATH.as_ptr()) {
        return fail(b"afc: DAM fan module is not loaded\n");
    }

    let mut product = [0u8; 96];
    let Some(product_len) = read_once(PRODUCT_PATH.as_ptr(), &mut product) else {
        return fail(b"afc: cannot read product name\n");
    };
    let Some(range) = detect_range(&product[..product_len]) else {
        return fail(b"afc: unknown fan range; configure /etc/afc.conf\n");
    };

    let fans = [open_fan(FAN1_PATH), open_fan(FAN2_PATH)];
    if fans[0] < 0 && fans[1] < 0 {
        return fail(b"afc: no valid fan character devices\n");
    }

    let mut sensors = [Sensor { fd: -1 }; MAX_SENSORS];
    let sensor_count = discover_sensors(&mut sensors);
    if sensor_count == 0 {
        close(fans[0]);
        close(fans[1]);
        return fail(b"afc: no trusted CPU/GPU temperature sensor\n");
    }
    if !install_signal(2) || !install_signal(15) {
        return fail(b"afc: cannot install signal handlers\n");
    }

    let status_fd = open(
        STATUS_PATH.as_ptr(),
        O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC | O_NOFOLLOW,
        0o644,
    );
    let mut speed = range.restore;
    let mut down_since = -1i64;
    let mut report_at = 0i64;

    while !STOP.load(Ordering::Relaxed) {
        let now = monotonic_sec();
        let temp = hottest(&sensors, sensor_count);
        let target = temp.map_or(range.max, |value| curve(value, range));

        if target >= speed {
            speed = target;
            down_since = -1;
        } else if down_since < 0 {
            down_since = now;
        } else if now - down_since >= DOWN_DELAY_SEC {
            speed = speed.saturating_sub(range.step).max(target);
        }

        if !set_fans(&fans, speed) {
            let _ = set_fans(&fans, range.max);
            return fail(b"afc: fan write failed; requested maximum\n");
        }
        let mut line = [0u8; 64];
        let n = status_line(temp.unwrap_or(0), speed, sensor_count, &mut line);
        let report = report_at == 0 || now >= report_at;
        publish(status_fd, &line[..n], report);
        if report {
            report_at = now.saturating_add(REPORT_SEC);
        }
        sleep_poll();
    }

    let restored = set_fans(&fans, range.restore);
    for sensor in &sensors[..sensor_count] {
        close(sensor.fd);
    }
    close(fans[0]);
    close(fans[1]);
    close(status_fd);
    if restored {
        0
    } else {
        fail(b"afc: failed to restore driver default\n")
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    let message = b"afc: internal invariant failure\n";
    let _ = call(
        SYS_WRITE,
        2,
        message.as_ptr() as usize,
        message.len(),
        0,
        0,
        0,
    );
    let _ = call(SYS_EXIT_GROUP, 70, 0, 0, 0, 0, 0);
    loop {
        core::hint::spin_loop();
    }
}
