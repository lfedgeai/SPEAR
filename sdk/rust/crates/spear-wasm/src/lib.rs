//! Safe, ergonomic Rust wrappers for Spear WASM hostcalls
//! 面向 Rust 的安全/易用 Spear WASM hostcall 封装
//!
//! Design goal / 设计目标：
//! - Keep this crate independent from any JS engine.
//! - 该 crate 与任何 JS 引擎解耦。
//!
//! ABI note / ABI 说明：
//! - All hostcalls use pointers into WASM linear memory (32-bit).
//! - 所有 hostcall 都通过 32 位指针访问 WASM 线性内存。

#![deny(unsafe_op_in_unsafe_fn)]

use thiserror::Error;

pub use spear_wasm_sys::constants;

#[derive(Debug, Clone, Error)]
#[error("{op}: {code} (errno={errno})")]
pub struct SpearError {
    /// Stable error discriminator / 稳定错误码（可判别）
    pub code: &'static str,
    /// Raw negative errno returned by hostcalls / hostcall 返回的负 errno
    pub errno: i32,
    /// Operation name / 操作名
    pub op: &'static str,
}

impl SpearError {
    pub fn is_code(&self, code: &str) -> bool {
        self.code == code
    }
}

fn errno_to_code(errno: i32) -> &'static str {
    match -errno {
        libc::EBADF => "invalid_fd",
        libc::EFAULT => "invalid_ptr",
        libc::ENOSPC => "buffer_too_small",
        libc::EINVAL => "invalid_cmd",
        libc::EIO => "internal",
        libc::EAGAIN => "eagain",
        _ => "unknown",
    }
}

fn rc_to_result(rc: i32, op: &'static str) -> Result<i32, SpearError> {
    if rc >= 0 {
        Ok(rc)
    } else {
        Err(SpearError {
            code: errno_to_code(rc),
            errno: rc,
            op,
        })
    }
}

fn rc_to_unit(rc: i32, op: &'static str) -> Result<(), SpearError> {
    rc_to_result(rc, op).map(|_| ())
}

fn cast_ptr_len(bytes: &[u8]) -> (i32, i32) {
    let ptr = bytes.as_ptr() as usize as i32;
    let len = bytes.len() as i32;
    (ptr, len)
}

pub const LOG_TRACE: i32 = 0;
pub const LOG_DEBUG: i32 = 1;
pub const LOG_INFO: i32 = 2;
pub const LOG_WARN: i32 = 3;
pub const LOG_ERROR: i32 = 4;

pub fn log_write(level: i32, msg: &str) -> Result<(), SpearError> {
    #[cfg(target_arch = "wasm32")]
    {
        let b = msg.as_bytes();
        let (ptr, len) = cast_ptr_len(b);
        let rc = unsafe { spear_wasm_sys::log(level, ptr, len) };
        rc_to_unit(rc, "log")
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        match level {
            LOG_ERROR | LOG_WARN => eprintln!("{msg}"),
            _ => println!("{msg}"),
        }
        Ok(())
    }
}

pub fn log_trace(msg: &str) -> Result<(), SpearError> {
    log_write(LOG_TRACE, msg)
}

pub fn log_debug(msg: &str) -> Result<(), SpearError> {
    log_write(LOG_DEBUG, msg)
}

pub fn log_info(msg: &str) -> Result<(), SpearError> {
    log_write(LOG_INFO, msg)
}

pub fn log_warn(msg: &str) -> Result<(), SpearError> {
    log_write(LOG_WARN, msg)
}

pub fn log_error(msg: &str) -> Result<(), SpearError> {
    log_write(LOG_ERROR, msg)
}

fn recv_alloc_with<F>(
    op: &'static str,
    mut call: F,
    initial_cap: usize,
    max_attempts: usize,
) -> Result<Vec<u8>, SpearError>
where
    F: FnMut(*mut u8, &mut u32) -> i32,
{
    let mut cap = initial_cap.max(1);
    for _ in 0..max_attempts {
        let mut buf = vec![0u8; cap];
        let mut len = cap as u32;
        let rc = call(buf.as_mut_ptr(), &mut len);
        if rc >= 0 {
            let out_len = len as usize;
            buf.truncate(out_len);
            return Ok(buf);
        }
        if rc != -libc::ENOSPC {
            return Err(SpearError {
                code: errno_to_code(rc),
                errno: rc,
                op,
            });
        }

        cap = (len as usize).max(cap.saturating_mul(2));
    }

    Err(SpearError {
        code: "buffer_too_small",
        errno: -libc::ENOSPC,
        op,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fd(i32);

impl Fd {
    pub fn raw(self) -> i32 {
        self.0
    }
}

/// Close a chat-related FD (session or response)
/// 关闭一个 chat 相关 FD（会话或响应）
pub fn cchat_close(fd: Fd) -> Result<(), SpearError> {
    #[cfg(target_arch = "wasm32")]
    {
        let rc = unsafe { spear_wasm_sys::cchat_close(fd.0) };
        rc_to_unit(rc, "cchat_close")
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = fd;
        Err(SpearError {
            code: "unsupported_target",
            errno: -libc::ENOSYS,
            op: "cchat_close",
        })
    }
}

/// Chat session wrapper / Chat 会话封装
pub struct ChatSession {
    fd: Fd,
}

impl ChatSession {
    /// Create a new chat session / 创建新的 chat 会话
    pub fn create() -> Result<Self, SpearError> {
        #[cfg(target_arch = "wasm32")]
        {
            let fd = unsafe { spear_wasm_sys::cchat_create() };
            let fd = rc_to_result(fd, "cchat_create")?;
            return Ok(Self { fd: Fd(fd) });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            Err(SpearError {
                code: "unsupported_target",
                errno: -libc::ENOSYS,
                op: "cchat_create",
            })
        }
    }

    pub fn fd(&self) -> Fd {
        self.fd
    }

    /// Write one message into the session / 写入一条消息
    pub fn write_message(&mut self, role: &str, content: &str) -> Result<(), SpearError> {
        #[cfg(target_arch = "wasm32")]
        {
            let role_b = role.as_bytes();
            let content_b = content.as_bytes();
            let (role_ptr, role_len) = cast_ptr_len(role_b);
            let (content_ptr, content_len) = cast_ptr_len(content_b);
            let rc = unsafe {
                spear_wasm_sys::cchat_write_msg(self.fd.0, role_ptr, role_len, content_ptr, content_len)
            };
            return rc_to_unit(rc, "cchat_write_msg");
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (role, content);
            Err(SpearError {
                code: "unsupported_target",
                errno: -libc::ENOSYS,
                op: "cchat_write_msg",
            })
        }
    }

    /// Set one parameter by JSON (`{"key":...,"value":...}`)
    /// 通过 JSON 设置一个参数（`{"key":...,"value":...}`）
    pub fn set_param_json(&mut self, json: &str) -> Result<(), SpearError> {
        #[cfg(target_arch = "wasm32")]
        {
            let json_b = json.as_bytes();
            let (json_ptr, _json_len) = cast_ptr_len(json_b);
            let mut len_u32: u32 = json_b.len() as u32;
            let len_ptr = (&mut len_u32 as *mut u32) as usize as i32;
            let rc = unsafe {
                spear_wasm_sys::cchat_ctl(
                    self.fd.0,
                    constants::SPEAR_CCHAT_CTL_SET_PARAM,
                    json_ptr,
                    len_ptr,
                )
            };
            return rc_to_unit(rc, "cchat_ctl");
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = json;
            Err(SpearError {
                code: "unsupported_target",
                errno: -libc::ENOSYS,
                op: "cchat_ctl",
            })
        }
    }

    /// Register one tool function by offset + JSON schema
    /// 通过 offset + JSON schema 注册一个工具函数
    pub fn write_fn(&mut self, fn_offset: i32, fn_json: &str) -> Result<(), SpearError> {
        #[cfg(target_arch = "wasm32")]
        {
            let fn_b = fn_json.as_bytes();
            let (fn_ptr, fn_len) = cast_ptr_len(fn_b);
            let rc = unsafe { spear_wasm_sys::cchat_write_fn(self.fd.0, fn_offset, fn_ptr, fn_len) };
            return rc_to_unit(rc, "cchat_write_fn");
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (fn_offset, fn_json);
            Err(SpearError {
                code: "unsupported_target",
                errno: -libc::ENOSYS,
                op: "cchat_write_fn",
            })
        }
    }

    /// Send the chat request and return a response FD
    /// 发送 chat 请求并返回响应 FD
    pub fn send(&mut self, flags: i32) -> Result<Fd, SpearError> {
        #[cfg(target_arch = "wasm32")]
        {
            let resp_fd = unsafe { spear_wasm_sys::cchat_send(self.fd.0, flags) };
            let resp_fd = rc_to_result(resp_fd, "cchat_send")?;
            return Ok(Fd(resp_fd));
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = flags;
            Err(SpearError {
                code: "unsupported_target",
                errno: -libc::ENOSYS,
                op: "cchat_send",
            })
        }
    }

    /// Close the session FD
    /// 关闭会话 FD
    pub fn close(self) -> Result<(), SpearError> {
        cchat_close(self.fd)
    }
}

/// Read response bytes with ENOSPC-capacity growth
/// 通过 ENOSPC 扩容循环读取响应字节
pub fn cchat_recv_alloc(response_fd: Fd) -> Result<Vec<u8>, SpearError> {
    #[cfg(target_arch = "wasm32")]
    {
        recv_alloc_with(
            "cchat_recv",
            |out_ptr, out_len| unsafe {
                let out_ptr_i32 = out_ptr as usize as i32;
                let out_len_ptr_i32 = (out_len as *mut u32) as usize as i32;
                spear_wasm_sys::cchat_recv(response_fd.0, out_ptr_i32, out_len_ptr_i32)
            },
            64 * 1024,
            3,
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = response_fd;
        Err(SpearError {
            code: "unsupported_target",
            errno: -libc::ENOSYS,
            op: "cchat_recv",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_errno_mapping_known() {
        assert_eq!(errno_to_code(-libc::EBADF), "invalid_fd");
        assert_eq!(errno_to_code(-libc::ENOSPC), "buffer_too_small");
        assert_eq!(errno_to_code(-libc::EAGAIN), "eagain");
    }

    #[test]
    fn test_recv_alloc_grows_on_enospc() {
        let payload = b"hello world".to_vec();
        let mut called = 0usize;
        let out = recv_alloc_with(
            "fake_recv",
            |out_ptr, out_len| {
                called += 1;
                if called == 1 {
                    *out_len = 128;
                    return -libc::ENOSPC;
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(payload.as_ptr(), out_ptr, payload.len());
                }
                *out_len = payload.len() as u32;
                payload.len() as i32
            },
            8,
            3,
        )
        .unwrap();

        assert_eq!(called, 2);
        assert_eq!(out, payload);
    }
}
