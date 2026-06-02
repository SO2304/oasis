//! Gap 2 FHSS quantitative measurement. Runs 4 scenarios K=10 each,
//! prints the delivery ratios with bands. Used to produce the numbers
//! that go into SHADOW_AUDIT_GAP2_FHSS.md.

use oasis_lora_transport::fhss::channelized::ChannelizedRadio;
use oasis_lora_transport::fhss::FhssRadio;
use oasis_lora_transport::LoRaRadio;

fn trial(seed: [u8; 32], n_channels: u32, jam_chan: Option<u32>,
         jam_count: u32, packets: u32) -> f64 {
    let (radio_a, radio_b, bus) = ChannelizedRadio::pair(n_channels);
    if let Some(j) = jam_chan {
        // Primary jam channel + (jam_count - 1) additional jammed ones.
        for i in 0..jam_count { bus.set_jammed((j + i) % n_channels, true); }
    }

    let mut tx = FhssRadio::new(radio_a, seed, n_channels);
    let mut rx = FhssRadio::new(radio_b, seed, n_channels);

    let mut got = 0u32;
    for i in 0..packets {
        let _ = tx.tx_hopped(format!("{}", i).as_bytes()).unwrap();
        let mut buf = [0u8; 64];
        if rx.rx_hopped(&mut buf, 20).is_ok() { got += 1; }
    }
    got as f64 / packets as f64
}

fn banded(label: &str, seed_base: u8, n_channels: u32, jam: Option<u32>,
          jam_count: u32, packets: u32) {
    let mut ratios = Vec::new();
    for k in 0..10 {
        let seed = [seed_base.wrapping_add(k); 32];
        ratios.push(trial(seed, n_channels, jam, jam_count, packets));
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (mn, med, mx) = (ratios[0], ratios[5], ratios[9]);
    let mean: f64 = ratios.iter().sum::<f64>() / 10.0;
    let spread = (mx - mn) / 2.0;
    println!("{:50}  K=10  min={:.3} med={:.3} max={:.3}  mean={:.3}  half-spread=±{:.3}",
             label, mn, med, mx, mean, spread);
}

fn main() {
    println!("OASIS Gap 2 — FHSS delivery ratios (packets per trial: 80)\n");

    banded("no jammer, 1 chan (baseline)",
           0x11, 1, None,     0, 80);
    banded("no jammer, 8 chan FHSS",
           0x22, 8, None,     0, 80);
    banded("JAM 1 of 1 chan (no FHSS possible)",
           0x33, 1, Some(0),  1, 80);
    banded("JAM 1 of 8 chan, FHSS",
           0x44, 8, Some(3),  1, 80);
    banded("JAM 2 of 8 chan, FHSS",
           0x55, 8, Some(3),  2, 80);
    banded("JAM 4 of 8 chan, FHSS",
           0x66, 8, Some(0),  4, 80);
    banded("JAM 1 of 16 chan, FHSS",
           0x77, 16, Some(7), 1, 80);
    banded("JAM 1 of 32 chan, FHSS",
           0x88, 32, Some(15),1, 80);
}
