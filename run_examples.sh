#!/bin/bash

MEMBERS=$(taplo get -f examples/Cargo.toml 'workspace.members')
# TODO(bing): add debug
PROFILES=("release")

failed=""
skipped=""

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
                "panic" )
                    echo "(mozak-cli) skipping (${profile}): ${bin}"
                    skipped="${skipped}${bin} (${profile})\n"
                    continue
                    ;;
                # For this, we skip without writing to skipped because we
                # run the native version along with the zkvm version.
                "merkleproof-trustedroot-native" )
                    echo "(mozak-cli) skipping (${profile}): ${bin}"
                    continue
                    ;;
                "fibonacci-input" )
                    private_iotape="examples/${member}/iotape_private"
                    public_iotape="examples/${member}/iotape_public"
                    ;;
                "merkleproof-trustedroot" )
                    host_target=$(rustc --version --verbose | grep 'host' | cut -d ' ' -f2)
                    cargo run --manifest-path=examples/${bin}/Cargo.toml --release --features="native" --bin merkleproof-trustedroot-native --target $host_target

                    private_iotape="examples/${member}/private_input.tape"
                    public_iotape="examples/${member}/public_input.tape"
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

if [ -n "$skipped" ]; then
    echo -e  "\nSome tests were skipped:\n${skipped}"
    exit 0 
fi


if [ -n "$failed" ]; then
    echo -e  "\nSome tests failed:\n${failed}"
    exit 1
fi

exit 0
