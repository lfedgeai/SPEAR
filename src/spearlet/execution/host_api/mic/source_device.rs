use crate::spearlet::execution::host_api::DefaultHostApi;
use crate::spearlet::execution::hostcall::types::MicConfig;

#[allow(dead_code)]
pub(super) struct DeviceMicStartRequest {
    pub fd: i32,
    pub config: MicConfig,
    pub device_name: Option<String>,
}

#[allow(dead_code)]
pub(super) enum DeviceMicStartError {
    NotImplemented,
}

impl DefaultHostApi {
    #[allow(dead_code)]
    pub(super) fn spawn_mic_device_task(&self, _req: DeviceMicStartRequest) -> Result<(), DeviceMicStartError> {
        Err(DeviceMicStartError::NotImplemented)
    }
}
