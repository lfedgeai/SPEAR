use crate::spearlet::execution::host_api::DefaultHostApi;
use crate::spearlet::execution::hostcall::types::{FdEntry, FdInner, PollEvents, RtAsrConnState};

impl DefaultHostApi {
    pub(super) fn recompute_rtasr_readiness_locked(&self, e: &mut FdEntry) {
        let FdInner::RtAsr(st) = &e.inner else {
            return;
        };

        let mut mask = PollEvents::EMPTY;
        if !st.recv_queue.is_empty() {
            mask.insert(PollEvents::IN);
        }

        let writable = st.send_queue_bytes < st.max_send_queue_bytes
            && st.state != RtAsrConnState::Draining
            && st.state != RtAsrConnState::Closed
            && st.state != RtAsrConnState::Error
            && !e.closed;
        if writable {
            mask.insert(PollEvents::OUT);
        }

        if st.state == RtAsrConnState::Error {
            mask.insert(PollEvents::ERR);
        }

        if e.closed || st.state == RtAsrConnState::Closed {
            mask.insert(PollEvents::HUP);
        }

        e.poll_mask = mask;
    }
}

