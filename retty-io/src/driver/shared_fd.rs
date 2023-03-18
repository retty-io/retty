#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::{cell::UnsafeCell, io, rc::Rc};

use super::CURRENT;

// Tracks in-flight operations on a file descriptor. Ensures all in-flight
// operations complete before submitting the close.
#[derive(Clone, Debug)]
pub(crate) struct SharedFd {
    inner: Rc<Inner>,
}

struct Inner {
    // Open file descriptor
    #[cfg(unix)]
    fd: RawFd,

    #[cfg(windows)]
    fd: RawHandle,

    // Waker to notify when the close operation completes.
    state: UnsafeCell<State>,
}

enum State {
    Legacy(Option<usize>),
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner").field("fd", &self.fd).finish()
    }
}

#[cfg(unix)]
impl AsRawFd for SharedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.raw_fd()
    }
}

#[cfg(windows)]
impl AsRawHandle for SharedFd {
    fn as_raw_handle(&self) -> RawHandle {
        self.raw_handle()
    }
}

impl SharedFd {
    #[cfg(unix)]
    #[allow(unreachable_code, unused)]
    pub(crate) fn new(fd: RawFd) -> io::Result<SharedFd> {
        const RW_INTERESTS: mio::Interest = mio::Interest::READABLE.add(mio::Interest::WRITABLE);

        let state = {
            let reg = CURRENT.with(|inner| match inner {
                super::Inner::Legacy(inner) => {
                    let mut source = mio::unix::SourceFd(&fd);
                    super::legacy::LegacyDriver::register(inner, &mut source, RW_INTERESTS)
                }
            });

            State::Legacy(Some(reg?))
        };

        #[allow(unreachable_code)]
        Ok(SharedFd {
            inner: Rc::new(Inner {
                fd,
                state: UnsafeCell::new(state),
            }),
        })
    }

    #[cfg(windows)]
    pub(crate) fn new(fd: RawHandle) -> io::Result<SharedFd> {
        unimplemented!()
    }

    #[cfg(unix)]
    #[allow(unreachable_code, unused)]
    pub(crate) fn new_without_register(fd: RawFd) -> SharedFd {
        let state = CURRENT.with(|inner| match inner {
            super::Inner::Legacy(_) => State::Legacy(None),
        });

        SharedFd {
            inner: Rc::new(Inner {
                fd,
                state: UnsafeCell::new(state),
            }),
        }
    }

    #[cfg(windows)]
    #[allow(unreachable_code, unused)]
    pub(crate) fn new_without_register(fd: RawHandle) -> io::Result<SharedFd> {
        unimplemented!()
    }

    #[cfg(unix)]
    /// Returns the RawFd
    pub(crate) fn raw_fd(&self) -> RawFd {
        self.inner.fd
    }

    #[cfg(windows)]
    /// Returns the RawHandle
    pub(crate) fn raw_handle(&self) -> RawHandle {
        self.inner.fd
    }

    #[cfg(unix)]
    /// Try unwrap Rc, then deregister if registered and return rawfd.
    /// Note: this action will consume self and return rawfd without closing it.
    pub(crate) fn try_unwrap(self) -> Result<RawFd, Self> {
        let fd = self.inner.fd;
        match Rc::try_unwrap(self.inner) {
            Ok(_inner) => {
                let state = unsafe { &*_inner.state.get() };

                #[allow(irrefutable_let_patterns)]
                if let State::Legacy(idx) = state {
                    if CURRENT.is_set() {
                        CURRENT.with(|inner| {
                            match inner {
                                super::Inner::Legacy(inner) => {
                                    // deregister it from driver(Poll and slab) and close fd
                                    if let Some(idx) = idx {
                                        let mut source = mio::unix::SourceFd(&fd);
                                        let _ = super::legacy::LegacyDriver::deregister(
                                            inner,
                                            *idx,
                                            &mut source,
                                        );
                                    }
                                }
                            }
                        })
                    }
                }
                Ok(fd)
            }
            Err(inner) => Err(Self { inner }),
        }
    }

    #[cfg(windows)]
    /// Try unwrap Rc, then deregister if registered and return rawfd.
    /// Note: this action will consume self and return rawfd without closing it.
    pub(crate) fn try_unwrap(self) -> Result<RawHandle, Self> {
        unimplemented!()
    }

    #[allow(unused)]
    pub(crate) fn registered_index(&self) -> Option<usize> {
        let state = unsafe { &*self.inner.state.get() };
        match state {
            State::Legacy(s) => *s,
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let fd = self.fd;
        let state = unsafe { &mut *self.state.get() };
        let State::Legacy(idx) = state;
        if CURRENT.is_set() {
            CURRENT.with(|inner| {
                match inner {
                    super::Inner::Legacy(inner) => {
                        // deregister it from driver(Poll and slab) and close fd
                        if let Some(idx) = idx {
                            let mut source = mio::unix::SourceFd(&fd);
                            let _ =
                                super::legacy::LegacyDriver::deregister(inner, *idx, &mut source);
                        }
                    }
                }
            })
        }
        let _ = unsafe { std::fs::File::from_raw_fd(fd) };
    }
}
