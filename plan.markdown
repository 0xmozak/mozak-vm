# Recover constraint debugging

At the moment, we have the `Stark` trait.  But we need an extra trait `Constrained` (or so) to give access to the constraints.

We probably want to make `Constrainted` a supertrait of `Stark`.  Ie `Constrainted implies `Stark`.

But the implementation has to go in the opposite direction?

Hmm, a bit more coplicated:

```rust
pub trait Constrainted /* snip */;

pub trait SuperStark: Stark + Constrainted {
    fn super_stark(&self) -> &Stark;
}
```

Then we need a wrapper that wraps a `Constrainted` and we implement `Stark` for that.  Then we get `SuperStark` for free.
