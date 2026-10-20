#!/bin/bash
# Measured, single-boot Phala CPU profile. Never source provider user_config.
# This is defense in depth on the pinned production OS, not a repair for a
# development image or for an already exposed root-login service.
set -euo pipefail
umask 077

covenant_prelaunch_fail() {
    printf 'Covenant ephemeral pre-launch refused: %s\n' "$1" >&2
    return 1
}

covenant_check_user_config() {
    local file="$1"
    [[ ! -e "$file" ]] && return 0
    [[ -f "$file" ]] || covenant_prelaunch_fail "user_config is not a regular file"
    # This input is not measured. Only reject injected credentials; never use
    # it to choose a key, command, image, endpoint, registry or gateway.
    jq -e 'type == "object" and
        ((.ssh_authorized_keys // "") == "" or .ssh_authorized_keys == []) and
        ((.root_password // "") == "") and
        ((.root_public_key // "") == "")' "$file" >/dev/null ||
        covenant_prelaunch_fail "unmeasured root credentials or invalid user_config"
}

covenant_check_root_env() {
    local name
    for name in DSTACK_ROOT_PASSWORD DSTACK_ROOT_PUBLIC_KEY DSTACK_AUTHORIZED_KEYS; do
        [[ -z "${!name-}" ]] || covenant_prelaunch_fail "unmeasured root credential environment"
    done
}

covenant_prelaunch_main() {
    [[ "$(id -u)" == 0 ]] || covenant_prelaunch_fail "requires guest root"
    local name unit path comm image state
    covenant_check_root_env
    covenant_check_user_config /dstack/user_config
    unset DSTACK_GATEWAY_DOMAIN DSTACK_APP_DOMAIN DSTACK_ROOT_PASSWORD DSTACK_ROOT_PUBLIC_KEY DSTACK_AUTHORIZED_KEYS

    # The pinned prod recipe omits SSH servers and removes login/getty. A
    # pre-launch shutdown cannot establish that an unexpected server was never
    # reachable earlier, so reject these binaries and any running login daemon.
    # Disabled BusyBox login/getty applets are not independent network servers;
    # reject running login processes and activation units below instead.
    for name in sshd dropbear telnetd inetd xinetd gdbserver; do
        if command -v "$name" >/dev/null 2>&1; then
            covenant_prelaunch_fail "unexpected host login or debug binary"
        fi
    done
    for path in /proc/[0-9]*/comm; do
        [[ -r "$path" ]] || continue
        IFS= read -r comm < "$path" || continue
        case "$comm" in
            sshd|dropbear|telnetd|inetd|xinetd|getty|agetty|login|gdbserver)
                covenant_prelaunch_fail "unexpected active host login or debug process" ;;
        esac
    done
    for unit in ssh.service ssh.socket sshd.service sshd.socket ssh@.service sshd@.service \
        dropbear.service dropbear.socket telnet.socket getty.target getty@.service \
        serial-getty@.service console-getty.service container-getty@.service \
        autovt@.service debug-shell.service systemd-logind.service; do
        state="$(systemctl show "$unit" --property=ActiveState --value)"
        if [[ "$state" != inactive ]]; then
            covenant_prelaunch_fail "unexpected active host login unit"
        fi
        systemctl mask --runtime "$unit" >/dev/null
    done
    for path in /home/root/.ssh/authorized_keys /home/root/.ssh/authorized_keys2 \
        /root/.ssh/authorized_keys /root/.ssh/authorized_keys2; do
        [[ ! -s "$path" ]] || covenant_prelaunch_fail "unexpected host root authorized key"
        rm -f "$path"
    done
    # Password and service protections affect the guest OS only. The lease's
    # separately confined container SSH service remains buyer-key authenticated.
    [[ -f /etc/shadow ]] || covenant_prelaunch_fail "missing shadow file"
    sed -i 's/^root:[^:]*:/root:!:/' /etc/shadow
    awk -F: '$1 == "root" { found = 1; if ($2 != "!") exit 1 } END { if (!found) exit 1 }' /etc/shadow

    [[ -r /proc/swaps ]] || covenant_prelaunch_fail "missing swap evidence"
    awk 'NR == 1 { if ($1 != "Filename") exit 1; next } { exit 1 } END { if (NR != 1) exit 1 }' /proc/swaps ||
        covenant_prelaunch_fail "guest swap is enabled or ambiguous"
    ulimit -c 0
    sysctl -w kernel.core_pattern=core >/dev/null
    sysctl -w fs.suid_dumpable=0 >/dev/null
    [[ "$(cat /proc/sys/kernel/core_pattern)" == core ]] || covenant_prelaunch_fail "unsafe core handler"
    [[ "$(cat /proc/sys/fs/suid_dumpable)" == 0 ]] || covenant_prelaunch_fail "unsafe privileged core policy"

    # Pull exact public content and fail closed. No provider credentials, remote
    # installer, tag resolution, opaque registry configuration or cache fallback.
    local images
    images="$(docker compose -f /dstack/docker-compose.yaml config --images)"
    [[ -n "$images" ]] || covenant_prelaunch_fail "no workload images"
    while IFS= read -r image; do
        [[ "$image" =~ ^ghcr\.io/open-covenant/covenant/compute-node-public@sha256:[0-9a-f]{64}$ ]] ||
            covenant_prelaunch_fail "workload image is not the pinned Covenant public image"
        docker pull "$image" >/dev/null
        docker image inspect "$image" --format '{{json .RepoDigests}}' |
            jq -e --arg reference "$image" 'index($reference) != null' >/dev/null ||
            covenant_prelaunch_fail "pulled image digest mismatch"
    done <<< "$images"
    printf '%s\n' 'Covenant ephemeral pre-launch controls applied'
}

covenant_prelaunch_main
