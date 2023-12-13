#!/bin/bash

MEMBERS=$(taplo get -f examples/Cargo.toml 'workspace.members')
# TODO(bing): add debug
PROFILES=("release")

for profile in ${PROFILES[@]}
do
    for member in ${MEMBERS}
    do
        BINS=$(taplo get -f examples/${member}/Cargo.toml 'bin.*.name')
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

            cargo run --bin mozak-cli run -vvv examples/target/riscv32im-mozak-zkvm-elf/${profile}/${bin} examples/${private_iotape} examples/${public_iotape}

            if [ $? != 0 ]; then
                echo -e  "\nRunning ${bin} (${profile}) failed"
                exit 1
            fi
        done
    done
done

exit 0
