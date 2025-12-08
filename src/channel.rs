//! Binary channel communication between Bevy and the host application

use bevy::ecs::resource::Resource;

#[cfg(not(target_arch = "wasm32"))]
use crossbeam_channel::{Receiver, Sender, unbounded};

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::collections::VecDeque;

/// Trait for bidirectional binary message passing
pub trait BinaryChannel: Send + Sync {
    /// Send a binary message to the other end
    fn send(&self, data: Vec<u8>);

    /// Receive a binary message from the other end (non-blocking)
    fn receive(&self) -> Option<Vec<u8>>;
}

// =============================================================================
// Native (non-WASM) implementation using crossbeam channels
// =============================================================================

#[cfg(not(target_arch = "wasm32"))]
/// Resource wrapping a platform-specific channel implementation
#[derive(Resource)]
pub struct HostChannel {
    sender: Sender<Vec<u8>>,
    receiver: Receiver<Vec<u8>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for HostChannel {
    fn default() -> Self {
        let (sender, receiver) = unbounded();
        Self { sender, receiver }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl HostChannel {
    /// Creates a new host channel with the given sender and receiver
    pub fn new(sender: Sender<Vec<u8>>, receiver: Receiver<Vec<u8>>) -> Self {
        Self { sender, receiver }
    }

    /// Send a message to the host
    pub fn send(&self, data: Vec<u8>) {
        let _ = self.sender.send(data);
    }

    /// Receive a message from the host (non-blocking)
    pub fn receive(&self) -> Option<Vec<u8>> {
        self.receiver.try_recv().ok()
    }

    /// Get a clone of the sender for use in FFI
    pub fn get_sender(&self) -> Sender<Vec<u8>> {
        self.sender.clone()
    }

    /// Get a clone of the receiver for use in FFI
    pub fn get_receiver(&self) -> Receiver<Vec<u8>> {
        self.receiver.clone()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl BinaryChannel for HostChannel {
    fn send(&self, data: Vec<u8>) {
        self.send(data);
    }

    fn receive(&self) -> Option<Vec<u8>> {
        self.receive()
    }
}

// =============================================================================
// WASM implementation using thread-local queues
// =============================================================================

#[cfg(target_arch = "wasm32")]
thread_local! {
    static HOST_TO_BEVY: RefCell<VecDeque<Vec<u8>>> = const { RefCell::new(VecDeque::new()) };
    static BEVY_TO_HOST: RefCell<VecDeque<Vec<u8>>> = const { RefCell::new(VecDeque::new()) };
}

#[cfg(target_arch = "wasm32")]
/// Resource for host communication on WASM (uses thread-local queues)
#[derive(Resource, Default)]
pub struct HostChannel;

#[cfg(target_arch = "wasm32")]
impl HostChannel {
    /// Send a message to the host (JavaScript)
    pub fn send(&self, data: Vec<u8>) {
        BEVY_TO_HOST.with(|queue| {
            queue.borrow_mut().push_back(data);
        });
    }

    /// Receive a message from the host (non-blocking)
    pub fn receive(&self) -> Option<Vec<u8>> {
        HOST_TO_BEVY.with(|queue| queue.borrow_mut().pop_front())
    }
}

#[cfg(target_arch = "wasm32")]
/// Send a message from host (JavaScript) to Bevy
pub fn host_send_to_bevy(data: Vec<u8>) {
    HOST_TO_BEVY.with(|queue| {
        queue.borrow_mut().push_back(data);
    });
}

#[cfg(target_arch = "wasm32")]
/// Receive a message from Bevy (for host/JavaScript)
pub fn host_receive_from_bevy() -> Option<Vec<u8>> {
    BEVY_TO_HOST.with(|queue| queue.borrow_mut().pop_front())
}
