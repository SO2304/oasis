//! Runnable demo: LoRa time-on-air + EU868 1% duty-cycle throttling.
//!
//!   cd oasis-lora-transport && cargo run --example duty_cycle
//!
//! Pure software, no radio. Prints the airtime cost of each spreading factor and
//! shows the DutyCycleThrottler pacing a node to stay legal on EU868. This is the
//! software half of regional compliance: a real SX1262 driver calls `try_send()`
//! before every TX and waits when it returns false.

use oasis_lora_transport::airtime::{airtime_us, DutyCycleThrottler};
use oasis_lora_transport::LoRaParams;

fn main() {
    let payload = 24; // a typical OASIS v0A mesh envelope, bytes
    println!(
        "Time-on-air for a {payload}-byte payload (BW 125 kHz, CR 4/5, CRC on, 8-sym preamble):"
    );
    for sf in 7..=12u8 {
        let p = LoRaParams {
            sf,
            bw_code: 4,
            ..LoRaParams::default()
        };
        println!(
            "  SF{sf:<2} -> {:>8.2} ms",
            airtime_us(&p, payload) as f64 / 1000.0
        );
    }

    println!("\nEU868 1% duty cycle — one SF7 packet attempted every 200 ms for 30 s:");
    let p = LoRaParams {
        sf: 7,
        bw_code: 4,
        ..LoRaParams::default()
    };
    let air = airtime_us(&p, payload);
    let mut dc = DutyCycleThrottler::eu868_1pct(0);
    let (mut now, mut legal, mut throttled) = (0u64, 0u32, 0u32);
    for _ in 0..150 {
        if dc.try_send(air, now) {
            legal += 1;
        } else {
            throttled += 1;
        }
        now += 200;
    }
    println!("  result: {legal} legal, {throttled} throttled");
    println!(
        "  each SF7 packet costs {:.1} ms airtime; budget = 1 s burst + 1% of elapsed time",
        air as f64 / 1000.0
    );
}
