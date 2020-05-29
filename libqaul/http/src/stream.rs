//! Handles stream responeses (at some point)

use async_trait::async_trait;
use libqaul_rpc::{Response, StreamResponder};
use std::sync::Arc;
use tracing::warn;

/// A streamer that can alert clients to new stream events
pub struct StreamResp {
    // TODO: implement internals
}

#[async_trait]
impl StreamResponder for StreamResp {
    async fn respond(self: Arc<Self>, r: Response) {
        warn!("Stream update unimplemented!");
    }
}
