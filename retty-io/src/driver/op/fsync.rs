use std::io;

use super::{super::shared_fd::SharedFd, Op, OpAble};
use crate::{driver::legacy::ready::Direction, syscall_u32};

pub(crate) struct Fsync {
    #[allow(unused)]
    fd: SharedFd,
    #[cfg(target_os = "linux")]
    data_sync: bool,
}

impl Op<Fsync> {
    pub(crate) fn fsync(fd: &SharedFd) -> io::Result<Op<Fsync>> {
        Op::submit_with(Fsync {
            fd: fd.clone(),
            #[cfg(target_os = "linux")]
            data_sync: false,
        })
    }

    pub(crate) fn datasync(fd: &SharedFd) -> io::Result<Op<Fsync>> {
        Op::submit_with(Fsync {
            fd: fd.clone(),
            #[cfg(target_os = "linux")]
            data_sync: true,
        })
    }
}

impl OpAble for Fsync {
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall_u32!(fsync(self.fd.raw_fd()))
    }

    #[cfg(all(target_os = "linux"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        if self.data_sync {
            syscall_u32!(fdatasync(self.fd.raw_fd()))
        } else {
            syscall_u32!(fsync(self.fd.raw_fd()))
        }
    }
}
