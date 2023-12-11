#!/bin/bash

MEMBERS=$(taplo get -f Cargo.toml 'workspace.members')
# TODO(bing): add debug
PROFILES=("release")

for profile in ${PROFILES[@]}
do
    for member in ${MEMBERS}
    do
        BINS=$(taplo get -f ${member}/Cargo.toml 'bin.*.name')
        for bin in ${BINS}
        do
            echo "(mozak-cli) running example (${profile}): ${bin}"
            private_iotape="iotape.txt"
            public_iotape="iotape.txt"
            case ${bin} in
                "panic" | "merkleproof-trustedroot-native" )
                    echo "(mozak-cli) skipping (${profile}): ${bin}"
                    continue
                    ;;
                "fibonacci-input" )
                    private_iotape="${member}/iotape_private"
                    public_iotape="${member}/iotape_public"
                    ;;
            esac

            eval "mozak-cli run -vvv target/riscv32im-mozak-zkvm-elf/${profile}/${bin} ${private_iotape} ${public_iotape}"
        done
    done
done
