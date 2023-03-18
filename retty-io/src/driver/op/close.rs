#[cfg(unix)]
use std::{io, os::unix::io::RawFd};

use super::{Op, OpAble};
use crate::{driver::legacy::ready::Direction, syscall_u32};

pub(crate) struct Close {
    #[cfg(unix)]
    fd: RawFd,
}

impl Op<Close> {
    #[allow(unused)]
    #[cfg(unix)]
    pub(crate) fn close(fd: RawFd) -> io::Result<Op<Close>> {
        Op::try_submit_with(Close { fd })
    }
}

impl OpAble for Close {
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall_u32!(close(self.fd))
    }
}
