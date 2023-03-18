use std::{ffi::CString, io, path::Path};

pub(super) fn cstr(p: &Path) -> io::Result<CString> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(CString::new(p.as_os_str().as_bytes())?)
    }
    #[cfg(windows)]
    {
        unimplemented!()
    }
}

/// Do syscall and return Result<T, std::io::Error>
#[cfg(unix)]
#[macro_export]
macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

#[cfg(windows)]
#[macro_export]
macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        unimplemented!()
    }};
}

/// Do syscall and return Result<T, std::io::Error>
#[macro_export]
macro_rules! syscall_u32 {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res as u32)
        }
    }};
}
