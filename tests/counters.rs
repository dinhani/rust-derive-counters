use std::sync::atomic::AtomicI32;
use std::sync::atomic::AtomicU64;
use std::thread;

use atomic_float::AtomicF64;
use derive_counters::Counters;
use portable_atomic::AtomicI128;
use portable_atomic::AtomicU128;

#[derive(Default, Counters)]
struct Shared {
    scanned: AtomicU64,
    balance: AtomicI32,
    ratio: AtomicF64,
    label: String,
}

#[derive(Default, Counters)]
struct Local {
    count: u32,
    balance: i64,
    total: f64,
}

#[derive(Default, Counters)]
struct Wide {
    shared: AtomicU128,
    signed: AtomicI128,
    local: u128,
}

#[derive(Default, Counters)]
struct Grouped {
    #[counters(group(name = scans), group(name = scanned))]
    scanned: AtomicU64,
    #[counters(group(name = scans))]
    platform_scanned: AtomicU64,
    #[counters(group(name = scanned))]
    total: AtomicU64,
}

#[test]
fn atomic_counts_across_threads() {
    let shared = Shared::default();

    thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| (0..1000).for_each(|_| shared.inc_scanned()));
        }
    });

    assert_eq!(shared.get_scanned(), 8000);
}

#[test]
fn atomic_integer_methods() {
    let shared = Shared::default();

    shared.add_balance(10);
    shared.sub_balance(15);
    shared.dec_balance();
    shared.inc_balance();
    shared.dec_balance();

    assert_eq!(shared.get_balance(), -6);
    shared.set_balance(3);
    assert_eq!(shared.get_balance(), 3);
}

#[test]
fn atomic_float_methods() {
    let shared = Shared::default();

    shared.inc_ratio();
    shared.add_ratio(2.5);
    shared.sub_ratio(0.25);
    shared.dec_ratio();

    assert_eq!(shared.get_ratio(), 2.25);
    shared.set_ratio(0.5);
    assert_eq!(shared.get_ratio(), 0.5);
}

#[test]
fn plain_integer_methods() {
    let mut local = Local::default();

    local.inc_count();
    local.add_count(9);
    local.dec_count();
    local.sub_count(3);
    local.sub_balance(7);

    assert_eq!(local.get_count(), 6);
    assert_eq!(local.get_balance(), -7);
    local.set_count(42);
    assert_eq!(local.get_count(), 42);
}

#[test]
fn plain_float_methods() {
    let mut local = Local::default();

    local.inc_total();
    local.add_total(2.5);
    local.sub_total(0.25);
    local.dec_total();

    assert_eq!(local.get_total(), 2.25);
    local.set_total(0.5);
    assert_eq!(local.get_total(), 0.5);
}

#[test]
fn integers_saturate() {
    let shared = Shared::default();
    let mut local = Local::default();

    shared.dec_scanned();
    shared.set_balance(i32::MAX);
    shared.add_balance(10);
    local.sub_count(5);
    local.set_balance(i64::MIN);
    local.dec_balance();

    assert_eq!(shared.get_scanned(), 0);
    assert_eq!(shared.get_balance(), i32::MAX);
    assert_eq!(local.get_count(), 0);
    assert_eq!(local.get_balance(), i64::MIN);
}

#[test]
fn wide_integers() {
    let mut wide = Wide::default();

    wide.set_shared(u128::MAX - 1);
    wide.add_shared(5);
    wide.dec_signed();
    wide.sub_signed(i128::MAX);
    wide.inc_local();
    wide.add_local(u64::MAX as u128);

    assert_eq!(wide.get_shared(), u128::MAX);
    assert_eq!(wide.get_signed(), i128::MIN);
    assert_eq!(wide.get_local(), u64::MAX as u128 + 1);
}

#[test]
fn groups_update_every_member() {
    let grouped = Grouped::default();

    grouped.inc_group_scans();
    grouped.add_group_scans(4);
    grouped.inc_group_scanned();
    grouped.set_platform_scanned(0);
    grouped.dec_group_scans();

    assert_eq!(grouped.get_scanned(), 5);
    assert_eq!(grouped.get_platform_scanned(), 0);
    assert_eq!(grouped.get_total(), 1);
    grouped.set_group_scans(7);
    assert_eq!(grouped.get_platform_scanned(), 7);
}

#[test]
fn other_fields_are_kept() {
    let shared = Shared { label: String::from("images"), ..Shared::default() };

    shared.inc_scanned();

    assert_eq!(shared.label, "images");
}
