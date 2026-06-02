//! OASIS — QR Spore roundtrip proof
//!
//! 1. Create digests (simulated phone experience)
//! 2. Encode as QR (base64)
//! 3. Decode into fresh mesh
//! 4. Verify digests survived the trip

use oasis_rt::federation::FederatedMesh;
use oasis_rt::spore;
use oasis_rt::vec::*;

fn main() {
    eprintln!("=== QR SPORE ROUNDTRIP TEST ===\n");

    // Simulate phone experience: train vibrations
    let mut phone = FederatedMesh::new();
    let mut train_vib = vz();
    train_vib[10] = 0.3;
    train_vib[11] = 0.1;
    train_vib[12] = -0.5;
    phone.pool_push_test(train_vib, 0.8, 1.0, 0.4); // positive: train is safe

    let mut train_brake = vz();
    train_brake[0] = 0.9;
    train_brake[10] = -0.7;
    phone.pool_push_test(train_brake, 0.6, -1.0, 0.7); // negative: sudden brake hurts

    let mut ambient = vz();
    ambient[30] = 0.3;
    ambient[35] = 0.2;
    ambient[50] = 0.1;
    phone.pool_push_test(ambient, 0.2, 1.0, 0.2); // positive: quiet train is fine

    eprintln!("  Phone digests: {}", phone.digest_count());

    // Encode as QR
    let qr = spore::encode_qr(&phone, 10).unwrap();
    eprintln!("  QR payload: {} chars", qr.len());
    eprintln!("  QR: {}\n", qr);

    // Decode into PC mesh (simulates scanning QR on another device)
    let mut pc = FederatedMesh::new();
    let loaded = spore::decode_qr(&mut pc, &qr, 0.6).unwrap();
    eprintln!("  PC received: {} digests (trust=0.6)", loaded);
    eprintln!("  PC total: {} digests\n", pc.digest_count());

    // Verify
    let mut pass = true;
    if loaded != 3 {
        eprintln!("  FAIL: expected 3, got {}", loaded);
        pass = false;
    }
    if pc.digest_count() != 3 {
        eprintln!("  FAIL: count {} != 3", pc.digest_count());
        pass = false;
    }

    // Save PC memory (it now carries phone experience)
    pc.save("/tmp/oasis-pc-with-phone.bin").unwrap();
    let size = std::fs::metadata("/tmp/oasis-pc-with-phone.bin").unwrap().len();
    eprintln!("  PC memory file: {} bytes", size);

    // Now simulate a THIRD device loading PC's memory
    let mut device_c = FederatedMesh::new();
    let carried = device_c.merge_foreign("/tmp/oasis-pc-with-phone.bin", 0.5).unwrap();
    eprintln!("  Device C received {} digests via store-and-forward", carried);
    eprintln!("  Device C never met the phone — but has its experience!\n");

    if carried != 3 {
        eprintln!("  FAIL: store-forward got {}", carried);
        pass = false;
    }

    if pass {
        eprintln!("  ✓ ALL PASS — QR spore + store-and-forward PROVED");
    } else {
        eprintln!("  ✗ FAILED");
        std::process::exit(1);
    }
    std::fs::remove_file("/tmp/oasis-pc-with-phone.bin").ok();
}
