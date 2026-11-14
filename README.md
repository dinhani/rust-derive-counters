# derive-counters

Derive that generates counter methods for the numeric and atomic fields of a struct.

`#[derive(Counters)]` generates, for each field:

| Method                | Does                 |
| --------------------- | -------------------- |
| `inc_<field>()`       | Adds one             |
| `dec_<field>()`       | Subtracts one        |
| `add_<field>(amount)` | Adds the amount      |
| `sub_<field>(amount)` | Subtracts the amount |
| `set_<field>(value)`  | Replaces the value   |
| `get_<field>()`       | Returns the value    |

- **Atomic fields** (`AtomicU8` to `AtomicU64`, `AtomicUsize`, `AtomicI8` to `AtomicI64`, `AtomicIsize`, `AtomicU128` and `AtomicI128` from `portable-atomic`, and `AtomicF32`, `AtomicF64` from `atomic_float`) get methods that take `&self`, with relaxed ordering, so threads can share the struct.
- **Numeric fields** (`u8` to `u128`, `usize`, `i8` to `i128`, `isize`, `f32`, `f64`) get methods that take `&mut self`.
- Other fields get no methods.

Integer methods saturate: they stop at the type's minimum or maximum instead of panicking or wrapping, so `dec_` on 0 stays 0.

## Install

```toml
[dependencies]
derive-counters = { git = "https://github.com/dinhani/rust-derive-counters", tag = "v0.1.0" }
```

## Usage

```rust
use std::sync::atomic::AtomicU64;

use derive_counters::Counters;

#[derive(Default, Counters)]
struct Totals {
    scanned: AtomicU64,
    failed: AtomicU64,
    saved: AtomicU64,
}

let totals = Totals::default();
totals.inc_scanned();
totals.add_saved(1024);
println!("{}", totals.get_saved());
```

## Groups

A field joins a group with `#[counters(group(name = <name>))]`, repeated for more than one group. Counters with the same group get methods that update all of them at once: `inc_group_<name>`, `dec_group_<name>`, `add_group_<name>`, `sub_group_<name>` and `set_group_<name>`. Each counter keeps its own methods. A group needs two or more counters of the same type.

```rust
#[derive(Default, Counters)]
struct Totals {
    #[counters(group(name = scans))]
    scanned: AtomicU64,
    #[counters(group(name = scans))]
    platform_scanned: AtomicU64,
}

totals.inc_group_scans();
// get_scanned() == 1, get_platform_scanned() == 1
```
