# Circuits Sub-crate

The Circuits sub-crate contains all the constraints used to enforce the correctness of the VM Trace. 

## Columns

Note that we make use of the column structure to name columns and prevent code clutter.

So in order to refer to column by name instead of the index, we make use of the View pattern, where every [T, ColumnSize] can be converted into a SubTableView struct with named fields. 