use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use dashmap::DashMap;

pub enum NetworkHandle {
    TcpStream(TcpStream),
    TcpListener(TcpListener),
}

#[derive(Clone, Default)]
pub struct NetworkContext {
    pub handles: Arc<DashMap<u64, NetworkHandle>>,
    pub next_id: Arc<std::sync::atomic::AtomicU64>,
}

impl NetworkContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, handle: NetworkHandle) -> u64 {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.handles.insert(id, handle);
        id
    }

    pub fn remove(&self, id: u64) -> Option<NetworkHandle> {
        self.handles.remove(&id).map(|(_, h)| h)
    }
}
