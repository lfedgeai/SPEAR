use crate::spearlet::execution::host_api::errno::EINVAL;

const SSF_MAGIC: [u8; 4] = *b"SPST";
const SSF_VERSION_V1: u16 = 1;
const SSF_HEADER_MIN: usize = 32;

pub(crate) fn parse_ssf_v1_header(frame: &[u8]) -> Result<(u32, u16), i32> {
    if frame.len() < SSF_HEADER_MIN {
        return Err(-EINVAL);
    }
    if frame[0..4] != SSF_MAGIC {
        return Err(-EINVAL);
    }
    let version = u16::from_le_bytes([frame[4], frame[5]]);
    if version != SSF_VERSION_V1 {
        return Err(-EINVAL);
    }
    let header_len = u16::from_le_bytes([frame[6], frame[7]]) as usize;
    if header_len < SSF_HEADER_MIN || frame.len() < header_len {
        return Err(-EINVAL);
    }
    let stream_id = u32::from_le_bytes([frame[12], frame[13], frame[14], frame[15]]);
    let meta_len = u32::from_le_bytes([frame[24], frame[25], frame[26], frame[27]]) as usize;
    let data_len = u32::from_le_bytes([frame[28], frame[29], frame[30], frame[31]]) as usize;
    let remain = frame.len().saturating_sub(header_len);
    if meta_len.saturating_add(data_len) != remain {
        return Err(-EINVAL);
    }
    let msg_type = u16::from_le_bytes([frame[8], frame[9]]);
    Ok((stream_id, msg_type))
}

pub(crate) fn build_ssf_v1_frame(
    stream_id: u32,
    msg_type: u16,
    meta: &[u8],
    data: &[u8],
) -> Vec<u8> {
    let header_len: u16 = SSF_HEADER_MIN as u16;
    let mut out = Vec::with_capacity(header_len as usize + meta.len() + data.len());
    out.extend_from_slice(&SSF_MAGIC);
    out.extend_from_slice(&SSF_VERSION_V1.to_le_bytes());
    out.extend_from_slice(&header_len.to_le_bytes());
    out.extend_from_slice(&msg_type.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&stream_id.to_le_bytes());
    out.extend_from_slice(&1u64.to_le_bytes());
    out.extend_from_slice(&(meta.len() as u32).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(meta);
    out.extend_from_slice(data);
    out
}
