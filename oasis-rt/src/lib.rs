//! OASIS-RT — Rust Kernel for the Open Agentic System
//!
//! Bio-inspired agentic kernel: tension fields, Hebbian/STDP synapses,
//! emotional modulation, reflex arcs, federated learning.
//!
//! Each module stays under 400 lines (R10).

#![cfg_attr(not(feature = "std"), no_std)]

// Heap types (Vec, String, Box) require the alloc crate on no_std targets.
// Edition 2021's prelude includes them implicitly when std is available.
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod audio;
pub mod branching;
pub mod dreams;
pub mod efference;
pub mod emotion;
pub mod fmath;
// federation: uses Ed25519 signing + state lookup + std collections
// for federation digest exchange. Host-only at present (would need
// a no_std refactor to ship on MCU).
#[cfg(feature = "std")]
pub mod federation;
pub mod hal;
pub mod hyper_state;
pub mod morpho;
// nerve: uses serde_json + std::env for sensor/actuator translation
// helpers. MCU users wire their own translation layer.
#[cfg(feature = "std")]
pub mod nerve;
pub mod reflex;
// spinal: reads /sys filesystem entries for hardware auto-detection.
// Linux/Android-only by nature.
#[cfg(feature = "std")]
pub mod spinal;
// spore: transport-layer (UdpSocket, fs, thread). Host-only.
pub mod actions;
pub mod actuation;
pub mod authority;
pub mod enrollment;
pub mod firmware;
pub mod fragment;
pub mod identity;
pub mod journal;
pub mod mesh;
pub mod mesh_revocation;
pub mod modbus_gateway;
pub mod modbus_tcp;
// mbtcp_net: the sockets the Modbus TCP layer was missing (pilot phase A). std only:
// std::net and blocking threads.
#[cfg(feature = "std")]
pub mod mbtcp_net;
// mbtcp_conf: the config both pilot binaries read. In the library so it is shared and
// testable, not duplicated in two bins.
#[cfg(feature = "std")]
pub mod mbtcp_conf;
pub mod nav;
pub mod ownership;
pub mod parameters;
pub mod quorum;
// sealed: OSE1, end-to-end confidentiality for a mesh payload (C13). Needs mesh_v10
// for the identity keys it derives from.
#[cfg(feature = "mesh_v10")]
pub mod sealed;
pub mod services;
#[cfg(feature = "std_env")]
pub mod spore;
pub mod spore_crypto;
#[cfg(test)]
mod test_support;
pub mod timers;
pub mod topics;
pub mod transforms;
pub mod tx_lease;
// mavlink_min: MAVLink v2 parser that uses std::io::Read/Write + time.
// MCU users typically have their own MAVLink stack tied to their UART.
#[cfg(feature = "std")]
pub mod mavlink_min;
// mavlink_order: a signed OASIS order carried inside a MAVLink V2_EXTENSION, and the
// arming rule that only acts on the gate's Act. Needs mavlink_min, so std.
#[cfg(all(feature = "std", feature = "mesh_v10"))]
pub mod mavlink_order;
pub mod synapse;
pub mod tension;
// transport: transport abstractions (UdpSocket, file, LoRa-stub) that
// need std::net + std::fs + std::thread.
#[cfg(feature = "std")]
pub mod transport;
pub mod vec;
pub mod vitality;
pub mod world_model;
