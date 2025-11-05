//! Binary channel communication between Bevy and the host application

use bevy::ecs::resource::Resource;
use crossbeam_channel::{Receiver, Sender, unbounded};

/// Trait for bidirectional binary message passing
pub trait BinaryChannel: Send + Sync {
    /// Send a binary message to the other end
    fn send(&self, data: Vec<u8>);

    /// Receive a binary message from the other end (non-blocking)
    fn receive(&self) -> Option<Vec<u8>>;
}

/// Resource wrapping a platform-specific channel implementation
#[derive(Resource)]
pub struct HostChannel {
    sender: Sender<Vec<u8>>,
    receiver: Receiver<Vec<u8>>,
}

impl Default for HostChannel {
    fn default() -> Self {
        let (sender, receiver) = unbounded();
        Self { sender, receiver }
    }
}

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

impl BinaryChannel for HostChannel {
    fn send(&self, data: Vec<u8>) {
        self.send(data);
    }

    fn receive(&self) -> Option<Vec<u8>> {
        self.receive()
    }
}
