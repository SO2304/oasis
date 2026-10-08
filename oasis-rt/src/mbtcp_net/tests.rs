use super::*;
use std::net::TcpListener;
use std::thread;

/// A round trip over a **real socket**, which is the whole point of this module: the
/// previous Modbus TCP layer had none.
#[test]
fn frame_roundtrip_over_a_real_socket() {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    let payload = vec![0xA5u8; 153]; // the size of a v0B envelope carrying an order
    let expect = payload.clone();

    let h = thread::spawn(move || {
        let (mut s, _) = srv.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let got = read_frame(&mut s).unwrap();
        assert_eq!(got, expect);
        write_frame(&mut s, b"ack").unwrap();
    });

    let mut c = connect_timeout(&addr, Duration::from_secs(5)).unwrap();
    write_frame(&mut c, &payload).unwrap();
    assert_eq!(read_frame(&mut c).unwrap(), b"ack".to_vec());
    h.join().unwrap();
}

/// A declared length over the bound is refused **before** the body is read, so a hostile
/// peer cannot choose how much memory this side reserves.
#[test]
fn an_oversized_length_prefix_is_refused_without_allocating() {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap().to_string();

    let h = thread::spawn(move || {
        let (mut s, _) = srv.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        // Declares 4 GiB and sends four bytes.
        s.write_all(&u32::MAX.to_be_bytes()).unwrap();
        s.write_all(b"junk").unwrap();
        let _ = s.flush();
    });

    let mut c = connect_timeout(&addr, Duration::from_secs(5)).unwrap();
    let err = read_frame(&mut c).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData, "must refuse the declared length");
    h.join().unwrap();
}

/// A zero length is refused too: it would otherwise be an empty envelope handed to the
/// verifier.
#[test]
fn a_zero_length_frame_is_refused() {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    let h = thread::spawn(move || {
        let (mut s, _) = srv.accept().unwrap();
        s.write_all(&0u32.to_be_bytes()).unwrap();
        let _ = s.flush();
    });
    let mut c = connect_timeout(&addr, Duration::from_secs(5)).unwrap();
    assert_eq!(read_frame(&mut c).unwrap_err().kind(), io::ErrorKind::InvalidData);
    h.join().unwrap();
}

/// Writing more than the bound is refused locally, before anything reaches the wire.
#[test]
fn an_oversized_payload_is_refused_before_sending() {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    let h = thread::spawn(move || {
        let _ = srv.accept();
    });
    let mut c = connect_timeout(&addr, Duration::from_secs(5)).unwrap();
    let too_big = vec![0u8; MAX_LINK_FRAME + 1];
    assert_eq!(write_frame(&mut c, &too_big).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    drop(c);
    h.join().unwrap();
}

/// A silent peer times out instead of hanging. Without this the HMI would wait forever,
/// which the pilot prompt forbids: a timeout must become an exception, never silence.
#[test]
fn a_silent_peer_times_out() {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    let h = thread::spawn(move || {
        let (_s, _) = srv.accept().unwrap();
        thread::sleep(Duration::from_millis(600)); // accepts, says nothing
    });
    let mut c = connect_timeout(&addr, Duration::from_millis(150)).unwrap();
    let err = read_frame(&mut c).unwrap_err();
    assert!(matches!(err.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut), "expected a timeout, got {:?}", err.kind());
    h.join().unwrap();
}

/// A Modbus exchange reads exactly what the MBAP header declares, so a peer that sends
/// two responses back to back does not desynchronise the stream.
#[test]
fn modbus_exchange_reads_exactly_one_response() {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    // Two FC06 echoes back to back.
    let one = [0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x11, 0x06, 0x00, 0x0A, 0x00, 0x7B];
    let h = thread::spawn(move || {
        let (mut s, _) = srv.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let _ = modbus_read_request(&mut s).unwrap();
        s.write_all(&one).unwrap();
        s.write_all(&one).unwrap();
        let _ = s.flush();
        thread::sleep(Duration::from_millis(100));
    });

    let mut c = connect_timeout(&addr, Duration::from_secs(5)).unwrap();
    let resp = modbus_exchange(&mut c, &one).unwrap();
    assert_eq!(resp.len(), 12, "exactly one response, not both");
    assert_eq!(&resp[..], &one[..]);
    h.join().unwrap();
}
