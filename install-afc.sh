#!/bin/bash
# SPDX-License-Identifier: MIT
set -Eeuo pipefail
IFS=$'\n\t'
umask 077
export LC_ALL=C
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

readonly SERVICE=afc.service
readonly BIN_DST=/usr/local/sbin/afc
readonly SRC_DIR=/usr/local/src/afc
readonly SRC_DST=${SRC_DIR}/afc.rs
readonly UNIT_DST=/etc/systemd/system/${SERVICE}
readonly CONFIG_DST=/etc/afc.conf
readonly WMI=/sys/bus/wmi/devices/7A4DDFE7-5B5D-40B4-8595-4408E0CC7F56
readonly MODULE=/sys/module/acer_nitro_gaming_driver2
readonly -a CONFLICTS=(
    fanctl-rust.service
    fanctl.service
    dam-fan-controls.service
    dam-fan-controls-daemon.service
)

STAGE=
RANGE=
START=1

die() {
    printf 'install-afc: %s\n' "$*" >&2
    exit 1
}

note() {
    printf 'install-afc: %s\n' "$*"
}

cleanup() {
    if [[ -n ${STAGE} && -d ${STAGE} ]]; then
        rm -rf -- "${STAGE}"
    fi
}
trap cleanup EXIT

usage() {
    cat <<'EOF'
Usage: sudo ./install-afc.sh [--range MIN:MAX:STEP:RESTORE] [--no-start]

Known ANV15-51 systems use the built-in 512:2560:128:512 range.
Unknown DAM-compatible models require --range; values are never probed.
EOF
}

while (( $# )); do
    case $1 in
        --range)
            (( $# >= 2 )) || die "--range requires MIN:MAX:STEP:RESTORE"
            RANGE=$2
            shift 2
            ;;
        --no-start)
            START=0
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            die "unknown argument: $1"
            ;;
    esac
done

(( EUID == 0 )) || die "run as root"
[[ $(uname -m) == x86_64 ]] || die "AFC currently supports x86-64 only"

for command in rustc cc readelf install systemctl flock mktemp; do
    command -v "${command}" >/dev/null 2>&1 || die "required command not found: ${command}"
done

exec 9>/run/lock/afc-install.lock
flock -n 9 || die "another AFC installation is running"

readonly SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
readonly SOURCE=${SCRIPT_DIR}/afc.rs
[[ -f ${SOURCE} && ! -L ${SOURCE} ]] || die "regular source file not found: ${SOURCE}"

[[ -r /sys/class/dmi/id/sys_vendor ]] || die "DMI vendor is unavailable"
read -r vendor </sys/class/dmi/id/sys_vendor
[[ ${vendor} == *Acer* ]] || die "unsupported vendor: ${vendor}"
read -r product </sys/class/dmi/id/product_name || die "DMI product name is unavailable"
[[ -e ${WMI} ]] || die "DAM gaming WMI interface is absent"

if [[ ! -d ${MODULE} ]]; then
    command -v modprobe >/dev/null 2>&1 || die "DAM module is absent and modprobe is unavailable"
    modprobe acer_nitro_gaming_driver2 ||
        die "cannot load acer_nitro_gaming_driver2; install the DAM WMI driver first"
fi
[[ -d ${MODULE} ]] || die "DAM module did not load"

if command -v udevadm >/dev/null 2>&1; then
    udevadm settle --timeout=10 || die "udev did not settle"
fi
for _ in {1..50}; do
    [[ -c /dev/fan1 || -c /dev/fan2 ]] && break
    sleep 0.1
done
[[ -c /dev/fan1 || -c /dev/fan2 ]] || die "no DAM fan character device found"

validate_range() {
    local text=$1 min max step restore extra
    IFS=: read -r min max step restore extra <<<"${text}"
    [[ -z ${extra:-} && -n ${min:-} && -n ${max:-} &&
       -n ${step:-} && -n ${restore:-} ]] || return 1
    [[ ${min} =~ ^[0-9]+$ && ${max} =~ ^[0-9]+$ &&
       ${step} =~ ^[0-9]+$ && ${restore} =~ ^[0-9]+$ ]] || return 1
    min=$((10#${min}))
    max=$((10#${max}))
    step=$((10#${step}))
    restore=$((10#${restore}))
    (( step > 0 && min >= 128 && max <= 2560 && min < max &&
       min % step == 0 && max % step == 0 &&
       restore >= min && restore <= max ))
}

range_from_config() {
    local line min= max= step= restore=
    [[ -f ${CONFIG_DST} && ! -L ${CONFIG_DST} ]] || return 1
    while IFS= read -r line || [[ -n ${line} ]]; do
        line=${line%$'\r'}
        case ${line} in
            fan_min=*) min=${line#fan_min=} ;;
            fan_max=*) max=${line#fan_max=} ;;
            fan_step=*) step=${line#fan_step=} ;;
            fan_restore=*) restore=${line#fan_restore=} ;;
        esac
    done <"${CONFIG_DST}"
    printf '%s:%s:%s:%s' "${min}" "${max}" "${step}" "${restore}"
}

if [[ -n ${RANGE} ]]; then
    validate_range "${RANGE}" || die "invalid range: ${RANGE}"
elif [[ -e ${CONFIG_DST} ]]; then
    existing_range=$(range_from_config) || die "existing config must be a regular non-symlink file"
    validate_range "${existing_range}" || die "existing ${CONFIG_DST} contains an invalid range"
elif [[ ${product} != *ANV15-51* ]]; then
    die "unknown model ${product}; specify --range MIN:MAX:STEP:RESTORE"
fi

STAGE=$(mktemp -d /var/tmp/afc-install.XXXXXX)
chmod 0700 "${STAGE}"

note "compiling AFC with rustc $(rustc --version)"
env -u RUSTFLAGS -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER +    rustc "${SOURCE}" +    --edition=2021 +    -C opt-level=z +    -C panic=abort +    -C lto=fat +    -C codegen-units=1 +    -C overflow-checks=yes +    -C strip=symbols +    -C relocation-model=static +    -C link-arg=-nostartfiles +    -C link-arg=-static +    -C link-arg=-no-pie +    -C link-arg=-Wl,--build-id=none +    -o "${STAGE}/afc"

[[ -x ${STAGE}/afc ]] || die "compiler did not produce an executable"
readelf -hW "${STAGE}/afc" | grep -q 'Machine:.*X86-64' ||
    die "compiled artifact has the wrong architecture"
if readelf -lW "${STAGE}/afc" | grep -q ' INTERP '; then
    die "compiled artifact unexpectedly requires a dynamic interpreter"
fi
chmod 0755 "${STAGE}/afc"

device_allow=
[[ -c /dev/fan1 ]] && device_allow+=$'DeviceAllow=/dev/fan1 rw\n'
[[ -c /dev/fan2 ]] && device_allow+=$'DeviceAllow=/dev/fan2 rw\n'

cat >"${STAGE}/afc.service" <<EOF
[Unit]
Description=AFC Acer fan controller
After=systemd-modules-load.service
ConditionPathExists=${WMI}
Conflicts=fanctl-rust.service fanctl.service dam-fan-controls.service dam-fan-controls-daemon.service
StartLimitIntervalSec=60
StartLimitBurst=5

[Service]
Type=simple
ExecStart=${BIN_DST}
Restart=on-failure
RestartSec=5
TimeoutStopSec=5
RuntimeDirectory=afc
RuntimeDirectoryMode=0755
UMask=0027
StandardOutput=journal
StandardError=journal
SyslogIdentifier=afc
NoNewPrivileges=yes
CapabilityBoundingSet=
AmbientCapabilities=
DevicePolicy=closed
${device_allow}ProtectSystem=strict
ReadWritePaths=/run/afc
ProtectHome=yes
PrivateTmp=yes
ProtectHostname=yes
ProtectClock=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectProc=invisible
ProcSubset=pid
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
RemoveIPC=yes
SystemCallArchitectures=native
SystemCallFilter=~@mount @swap @reboot @raw-io @privileged @resources @obsolete
SystemCallErrorNumber=EPERM

[Install]
WantedBy=multi-user.target
EOF
chmod 0644 "${STAGE}/afc.service"

if [[ -n ${RANGE} ]]; then
    IFS=: read -r min max step restore <<<"${RANGE}"
    cat >"${STAGE}/afc.conf" <<EOF
fan_min=${min}
fan_max=${max}
fan_step=${step}
fan_restore=${restore}
EOF
    chmod 0640 "${STAGE}/afc.conf"
fi

backup() {
    local src=$1 name=$2
    if [[ -e ${src} || -L ${src} ]]; then
        cp -a -- "${src}" "${STAGE}/backup-${name}"
        return 0
    fi
    return 1
}

restore() {
    local dst=$1 name=$2
    if [[ -e ${STAGE}/backup-${name} || -L ${STAGE}/backup-${name} ]]; then
        rm -f -- "${dst}"
        cp -a -- "${STAGE}/backup-${name}" "${dst}"
    else
        rm -f -- "${dst}"
    fi
}

backup "${BIN_DST}" bin || true
backup "${SRC_DST}" src || true
backup "${UNIT_DST}" unit || true
if [[ -n ${RANGE} ]]; then
    backup "${CONFIG_DST}" config || true
fi

old_afc_enabled=$(systemctl is-enabled "${SERVICE}" 2>/dev/null || true)
old_afc_active=$(systemctl is-active "${SERVICE}" 2>/dev/null || true)
declare -A old_enabled old_active
for unit in "${CONFLICTS[@]}"; do
    old_enabled["${unit}"]=$(systemctl is-enabled "${unit}" 2>/dev/null || true)
    old_active["${unit}"]=$(systemctl is-active "${unit}" 2>/dev/null || true)
done

install_atomic() {
    local src=$1 dst=$2 mode=$3 tmp=${dst}.new.$$
    install -D -o root -g root -m "${mode}" "${src}" "${tmp}"
    mv -f -- "${tmp}" "${dst}"
}

install_atomic "${STAGE}/afc" "${BIN_DST}" 0755
install_atomic "${SOURCE}" "${SRC_DST}" 0644
install_atomic "${STAGE}/afc.service" "${UNIT_DST}" 0644
if [[ -n ${RANGE} ]]; then
    install_atomic "${STAGE}/afc.conf" "${CONFIG_DST}" 0640
fi

systemctl daemon-reload

rollback() {
    note "activation failed; restoring previous AFC installation"
    systemctl disable --now "${SERVICE}" >/dev/null 2>&1 || true
    restore "${BIN_DST}" bin
    restore "${SRC_DST}" src
    restore "${UNIT_DST}" unit
    if [[ -n ${RANGE} ]]; then
        restore "${CONFIG_DST}" config
    fi
    systemctl daemon-reload
    [[ ${old_afc_enabled} == enabled ]] && systemctl enable "${SERVICE}" >/dev/null
    [[ ${old_afc_active} == active ]] && systemctl start "${SERVICE}" || true
    for unit in "${CONFLICTS[@]}"; do
        [[ ${old_enabled[${unit}]} == enabled ]] && systemctl enable "${unit}" >/dev/null
        [[ ${old_active[${unit}]} == active ]] && systemctl start "${unit}" || true
    done
}

if (( START )); then
    if ! systemctl enable --now "${SERVICE}"; then
        rollback
        journalctl -u "${SERVICE}" -n 30 --no-pager >&2 || true
        die "AFC activation failed"
    fi
    if ! systemctl is-active --quiet "${SERVICE}"; then
        rollback
        journalctl -u "${SERVICE}" -n 30 --no-pager >&2 || true
        die "AFC exited during activation"
    fi
    for unit in "${CONFLICTS[@]}"; do
        if [[ ${old_enabled[${unit}]} == enabled ]]; then
            systemctl disable "${unit}" >/dev/null ||
                die "AFC is active, but failed to disable conflicting ${unit}"
        fi
    done
    note "AFC is installed, enabled, and active"
else
    systemctl enable "${SERVICE}"
    note "AFC is installed and enabled; activation was skipped"
fi

note "binary: ${BIN_DST}"
note "service: ${UNIT_DST}"
note "status: /run/afc/status"
