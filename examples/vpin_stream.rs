//! Streaming VPIN from a CSV of trades on stdin, or from a built-in demo tape.
//!
//! ```text
//! cargo run --example vpin_stream                       # demo tape (also used when stdin is empty)
//! cargo run --example vpin_stream -- 2000 50 < trades.csv
//! ```
//!
//! CSV rows are `price,volume` (a header line is skipped). Arguments are the
//! bucket volume and the number of buckets; ELO (2012) use `ADV / 50` and 50.
//! Prints one line per completed bucket once VPIN is ready.

use std::io::{self, BufRead, IsTerminal};

use flowtox::Vpin;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bucket: f64 = args
        .first()
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(2_000.0);
    let n: usize = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(50);
    let mut vpin = Vpin::new(bucket, n)?;

    let mut trades: Vec<(f64, f64)> = if io::stdin().is_terminal() {
        Vec::new()
    } else {
        io::stdin()
            .lock()
            .lines()
            .map_while(|l| l.ok())
            .filter_map(|l| {
                let mut it = l.split(',');
                let p = it.next()?.trim().parse().ok()?;
                let v = it.next()?.trim().parse().ok()?;
                Some((p, v))
            })
            .collect()
    };
    if trades.is_empty() {
        trades = demo_tape();
    }

    let mut seen = 0;
    for (i, (p, v)) in trades.iter().enumerate() {
        let out = vpin.update(*p, *v);
        if vpin.completed_buckets() > seen {
            seen = vpin.completed_buckets();
            if let Some(x) = out {
                println!("trade {:>7}  bucket {:>5}  vpin {:.4}", i + 1, seen, x);
            }
        }
    }
    if !vpin.is_ready() {
        eprintln!(
            "only {} of {} buckets filled; VPIN not ready",
            vpin.completed_buckets(),
            n
        );
    }
    Ok(())
}

fn demo_tape() -> Vec<(f64, f64)> {
    let mut state: u64 = 99;
    let mut p = 50.0_f64;
    (0..40_000)
        .map(|i| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let r = (state >> 33) % 100;
            let up_odds = if (20_000..26_000).contains(&i) {
                90
            } else {
                50
            };
            p += if r < up_odds { 0.01 } else { -0.01 };
            (p, 10.0 + (r % 7) as f64 * 5.0)
        })
        .collect()
}
