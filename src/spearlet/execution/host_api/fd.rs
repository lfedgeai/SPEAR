use crate::spearlet::execution::host_api::DefaultHostApi;
use crate::spearlet::execution::hostcall::types::PollEvents;

impl DefaultHostApi {
    pub fn spear_ep_create(&self) -> i32 {
        self.fd_table.ep_create()
    }

    pub fn spear_ep_ctl(&self, epfd: i32, op: i32, fd: i32, events: i32) -> i32 {
        self.fd_table
            .ep_ctl(epfd, op, fd, PollEvents::from_bits_truncate(events as u32))
    }

    pub fn spear_ep_wait_ready(&self, epfd: i32, timeout_ms: i32) -> Result<Vec<(i32, i32)>, i32> {
        self.fd_table.ep_wait_ready(epfd, timeout_ms)
    }

    pub fn spear_ep_close(&self, epfd: i32) -> i32 {
        self.fd_table.close(epfd)
    }

    pub fn spear_fd_ctl(
        &self,
        fd: i32,
        cmd: i32,
        payload: Option<&[u8]>,
    ) -> Result<Option<Vec<u8>>, i32> {
        self.fd_table.fd_ctl(fd, cmd, payload)
    }
}

