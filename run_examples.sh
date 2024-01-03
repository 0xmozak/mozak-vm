#!/bin/bash

MEMBERS=$(taplo get -f examples/Cargo.toml 'workspace.members')
# TODO(bing): add debug
PROFILES=("release")

failed=""

for profile in ${PROFILES[@]}
do
    for member in ${MEMBERS}
    do
        BINS=$(taplo get -f examples/${member}/Cargo.toml 'bin.*.name')
        for bin in ${BINS}
        do
            echo "(mozak-cli) running example (${profile}): ${bin}"
            private_iotape=""
            public_iotape=""
            case ${bin} in
                # TODO(bing): fix to work with this script
                "panic" | "merkleproof-trustedroot-native" )
                    echo "(mozak-cli) skipping (${profile}): ${bin}"
                    continue
                    ;;
                "fibonacci-input" )
                    private_iotape="examples/${member}/iotape_private"
                    public_iotape="examples/${member}/iotape_public"
                    ;;
            esac

            cargo run --bin mozak-cli \
            run -vvv examples/target/riscv32im-mozak-zkvm-elf/${profile}/${bin} \
            ${private_iotape} \
            ${public_iotape}

            # cargo exits with 0 if success
            if [ $? != 0 ]; then
                failed="${failed}${bin} (${profile})\n"
            fi
        done
    done
done

if [ -n "$failed" ]; then
    echo -e  "\nSome tests failed:\n${failed}"
    exit 1
fi

exit 0
