//! The VPIN bucket boundary, the line most hand-rolled VPIN code gets wrong.
//!
//! ```text
//! cargo run --example block_print
//! ```
//!
//! Buckets of 1,000 shares, VPIN over 4 buckets. Four balanced buckets fill
//! first, then one 2,500-share buy prints. The block has to be split: 1,000
//! shares close one bucket, 1,000 the next, and 500 wait in the open bucket.
//! Code that drops the whole print into the current bucket and closes it gets
//! a bucket of 2,500 shares, and if it still divides by the nominal 1,000 its
//! "probability" comes out above 1. Code that throws the overflow away never
//! sees 1,500 of the 2,500 shares.

use flowtox::{Side, Vpin};

fn main() -> Result<(), flowtox::Error> {
    let mut vpin = Vpin::new(1_000.0, 4)?;
    for _ in 0..4 {
        vpin.update_classified(500.0, 500.0); // balanced: |B - S| = 0
    }
    let before = vpin.value().unwrap_or(0.0);
    let after = vpin.update_side(Side::Buy, 2_500.0).unwrap();
    println!("VPIN before the block: {before:.3}");
    println!("VPIN after a 2,500-share buy: {after:.3}"); // (0 + 0 + 1000 + 1000) / 4000 = 0.5
    println!("completed buckets: {}", vpin.completed_buckets());
    assert!((after - 0.5).abs() < 1e-12);
    assert!(after <= 1.0);
    Ok(())
}
