//! The single-surface lock, against real sockets.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};

use nerve_windows_surface::platform::instance::{Claim, claim};

/// A port nothing else in this suite will take.
fn free_port() -> u16 {
    let listener = TcpListener::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)))
        .expect("an ephemeral loopback port must be bindable");
    let port = listener.local_addr().expect("bound").port();
    drop(listener);
    port
}

#[test]
fn the_second_stands_down() {
    let port = free_port();
    let first = claim(port).expect("first claim");
    assert!(first.may_stream());

    let second = claim(port).expect("a taken port is an answer, not an error");
    assert!(matches!(second, Claim::Yield));
    assert!(
        !second.may_stream(),
        "a surface that does not own the lock must never open a stream — \
         it would hold the hub alive after the owner left"
    );
}

/// The lock cannot go stale: the OS releases the port when the process does,
/// however it ends. This is what a lock *file* gets wrong after a hard kill.
#[test]
fn releasing_the_lock_frees_it_for_the_next_process() {
    let port = free_port();
    let first = claim(port).expect("first claim");
    drop(first);

    let next = claim(port).expect("second claim");
    assert!(
        next.may_stream(),
        "the port stayed locked after its owner left"
    );
}

/// Nothing ever connects to the lock, so it must not be mistaken for a service.
#[test]
fn the_lock_listens_on_loopback_only() {
    let port = free_port();
    let Claim::Owned(listener) = claim(port).expect("claim") else {
        panic!("expected to own the lock");
    };
    let address = listener.local_addr().expect("bound");
    assert!(
        address.ip().is_loopback(),
        "binding anything but loopback would raise a firewall prompt: {address}"
    );
}
