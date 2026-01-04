use crate::spearlet::execution::host_api::DefaultHostApi;
use crate::spearlet::execution::hostcall::types::{FdEntry, FdInner, PollEvents};

impl DefaultHostApi {
    pub(super) fn recompute_mic_readiness_locked(&self, e: &mut FdEntry) {
        let FdInner::Mic(st) = &e.inner else {
            return;
        };

        let mut mask = PollEvents::EMPTY;
        if !st.queue.is_empty() {
            mask.insert(PollEvents::IN);
        }
        if st.last_error.is_some() {
            mask.insert(PollEvents::ERR);
        }
        if e.closed {
            mask.insert(PollEvents::HUP);
        }
        e.poll_mask = mask;
    }
}

