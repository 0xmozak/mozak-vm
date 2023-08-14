For `let loops = 100_000;`:

     Running benches/simple_prover.rs (/home/matthias/mozak/prog/mozak-vm/target/release/deps/simple_prover-17357cc5cf3ee275)
Benchmarking simple_prover/simple_prover: Warming up for 3.0000 s
Warning: Unable to complete 10 samples in 10.0s. You may wish to increase target time to 126.3s.
Benchmarking simple_prover/simple_prover: Collecting 10 samples in estimated 12
simple_prover/simple_prover
                        time:   [12.229 s 12.255 s 12.278 s]
                        change: [+3288.0% +3325.3% +3363.8%] (p = 0.00 < 0.05)

After ripping out all the limbs (best case we can hope for):

     Running benches/simple_prover.rs (/home/matthias/mozak/prog/mozak-vm/target/release/deps/simple_prover-17357cc5cf3ee275)
Benchmarking simple_prover/simple_prover: Warming up for 3.0000 s
Warning: Unable to complete 10 samples in 10.0s. You may wish to increase target time to 119.1s.
simple_prover/simple_prover
                        time:   [11.423 s 11.476 s 11.536 s]
                        change: [-6.8336% -6.3564% -5.8285%] (p = 0.00 < 0.05)

Weirdly enough, that's only a 6% improvement.  I would have expected more.

--- now with only 4 registers, and xor limb checks ripped out, but still with 
'let loops = 100_000;':

Running benches/simple_prover.rs (/home/matthias/mozak/prog/mozak-vm/target/release/deps/simple_prover-17357cc5cf3ee275)
Benchmarking simple_prover/simple_prover: Warming up for 3.0000 s
Warning: Unable to complete 10 samples in 10.0s. You may wish to increase target time to 59.9s.
simple_prover/simple_prover
                        time:   [5.9729 s 5.9897 s 6.0070 s]
                        change: [+2351.7% +2397.0% +2438.2%] (p = 0.00 < 0.05)
                        Performance has regressed.

----

Now this is twice as fast.  That's still a bit slower than expected, perhaps?  Because we lost almost a 100 columns?
