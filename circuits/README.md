# Circuits Sub-crate

The Circuits sub-crate contains all the constraints used to enforce the correctness of the VM Trace. 

## Columns

Note that we make use of the column structure to name columns and prevent code clutter.

So in order to refer to column by name instead of the index, we make use of the View pattern, where every [T, ColumnSize] can be converted into a SubTableView struct with named fields. 

## Constraints 

Q: Why do we not scope constraint in here, by adding in selectors? https://gist.github.com/ElusAegis/85df7c0027f4ad013359c8fac13605ff

A: **Matthias**
There's typically one of two reasons for that:
One, sometimes multiplying a selector into a constraint increases its degree above 3, which is what we currently picked as the upper limit.
Two, for simplicity we just make certain parts of our circuits always do their thing, instead of doing it conditionally. That makes things easier to debug and understand.
However, we can change that later, if something else turns out to be simpler, or if we find via benchmarking that another approach offers a big enough speed up to change.
(I suspect for gadgets that are internal to the table, turning them off when not in use won't increase our speed. But eg reducing the number of rows we have to raincheck or do a cross table lookup on might help with performance.
But that's just speculation. And until we run some comparative benchmarks, we go by whatever is simplest and easiest to understand.)