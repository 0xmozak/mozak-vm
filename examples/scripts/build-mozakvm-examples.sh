#!/bin/bash
for example in *; do
    if [ -d "$example" ]; then
        cd $example;
        for target in  *; do
            if [ "$target" == "mozakvm" ]; then
                cd mozakvm;
                cargo build-mozakvm;
                cd ..
            fi
        done
        cd ..
    fi
done