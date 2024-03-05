#!/bin/bash

set -euxo pipefail

MEMBERS=$(taplo get -f examples/Cargo.toml 'workspace.members')
# TODO(bing): add debug
PROFILES=("release")

failed=""
skipped=""

for profile in "${PROFILES[@]}"; do
    for member in ${MEMBERS}; do
        BINS=$(taplo get -f examples/"${member}"/Cargo.toml 'bin.*.name')
        for bin in ${BINS}; do
            echo "(mozak-cli) running example (${profile}): ${bin}"
            case ${bin} in
                # TODO(bing): fix to work with this script
                "panic")
                    echo "(mozak-cli) skipping (${profile}): ${bin}"
                    skipped="${skipped}${bin} (${profile})\n"
                    continue
                    ;;
                # For this, we skip without writing to skipped because we
                # run the native version along with the mozakvm version.
                "merkleproof-trustedroot-native" | "token-native" | "wallet-native" | "walletbin" | "tokenbin")
                    echo "(mozak-cli) skipping (${profile}): ${bin}"
                    continue
                    ;;
                "merkleproof-trustedroot")
                    echo "(mozak-cli) skipping (${profile}): ${bin}"
                    skipped="${skipped}${bin} (${profile})\n"
                    continue
                    ;;

            esac

            # shellcheck disable=SC2086
            # Double quoting the iotapes here is not what we want since we
            # want an empty argument if iotapes are not required.
            if ! cargo run --bin mozak-cli \
                run -vvv examples/target/riscv32im-mozak-mozakvm-elf/"${profile}"/"${bin}"; then
                failed="${failed}${bin} (${profile})\n"
            fi
        done
    done
done

if [ -n "$skipped" ]; then
    echo -e "\nSome tests were skipped:\n${skipped}"
fi

if [ -n "$failed" ]; then
    echo -e "\nSome tests failed:\n${failed}"
    exit 1
fi

exit 0
