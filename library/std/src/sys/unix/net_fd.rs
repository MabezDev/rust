#![unstable(reason = "not public", issue = "none", feature = "net_fd")]

use crate::cmp;
use crate::io::{self, Initializer, IoSlice, IoSliceMut, Read};
use crate::mem;
use crate::sys::{
    cvt,
    net::netc::{self, c_int, c_void},
};
use crate::sys_common::AsInner;

#[derive(Debug)]
#[rustc_layout_scalar_valid_range_start(0)]
// libstd/os/raw/mod.rs assures me that every libstd-supported platform has a
// 32-bit c_int. Below is -2, in two's complement, but that only works out
// because c_int is 32 bits.
#[rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FE)]
pub struct NetFileDesc {
    fd: c_int,
}

// The maximum read limit on most POSIX-like systems is `SSIZE_MAX`,
// with the man page quoting that if the count of bytes to read is
// greater than `SSIZE_MAX` the result is "unspecified".
//
// On macOS, however, apparently the 64-bit libc is either buggy or
// intentionally showing odd behavior by rejecting any read with a size
// larger than or equal to INT_MAX. To handle both of these the read
// size is capped on both platforms.
#[cfg(target_os = "macos")]
const READ_LIMIT: usize = c_int::MAX as usize - 1;
#[cfg(not(target_os = "macos"))]
const READ_LIMIT: usize = netc::ssize_t::MAX as usize;

#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "ios",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
))]
const fn max_iov() -> usize {
    netc::IOV_MAX as usize
}

#[cfg(any(target_os = "android", target_os = "emscripten", target_os = "linux"))]
const fn max_iov() -> usize {
    netc::UIO_MAXIOV as usize
}

#[cfg(not(any(
    target_os = "android",
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "freebsd",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
)))]
const fn max_iov() -> usize {
    16 // The minimum value required by POSIX.
}

impl NetFileDesc {
    pub fn new(fd: c_int) -> NetFileDesc {
        assert_ne!(fd, -1i32);
        // SAFETY: we just asserted that the value is in the valid range and isn't `-1` (the only value bigger than `0xFF_FF_FF_FE` unsigned)
        unsafe { NetFileDesc { fd } }
    }

    pub fn raw(&self) -> c_int {
        self.fd
    }

    /// Extracts the actual file descriptor without closing it.
    pub fn into_raw(self) -> c_int {
        let fd = self.fd;
        mem::forget(self);
        fd
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            netc::read(self.fd, buf.as_mut_ptr() as *mut c_void, cmp::min(buf.len(), READ_LIMIT))
        })?;
        Ok(ret as usize)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            netc::readv(
                self.fd,
                bufs.as_ptr() as *const netc::iovec,
                cmp::min(bufs.len(), max_iov()) as c_int,
            )
        })?;
        Ok(ret as usize)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        true
    }

    pub fn read_to_end(&self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let mut me = self;
        (&mut me).read_to_end(buf)
    }

    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        #[cfg(target_os = "android")]
        use super::android::cvt_pread64;

        #[cfg(not(target_os = "android"))]
        unsafe fn cvt_pread64(
            fd: c_int,
            buf: *mut c_void,
            count: usize,
            offset: i64,
        ) -> io::Result<isize> {
            #[cfg(not(target_os = "linux"))]
            use netc::pread as pread64;
            #[cfg(target_os = "linux")]
            use netc::pread64;
            cvt(pread64(fd, buf, count, offset))
        }

        unsafe {
            cvt_pread64(
                self.fd,
                buf.as_mut_ptr() as *mut c_void,
                cmp::min(buf.len(), READ_LIMIT),
                offset as i64,
            )
            .map(|n| n as usize)
        }
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            netc::write(self.fd, buf.as_ptr() as *const c_void, cmp::min(buf.len(), READ_LIMIT))
        })?;
        Ok(ret as usize)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            netc::writev(
                self.fd,
                bufs.as_ptr() as *const netc::iovec,
                cmp::min(bufs.len(), max_iov()) as c_int,
            )
        })?;
        Ok(ret as usize)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        true
    }

    pub fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        #[cfg(target_os = "android")]
        use super::android::cvt_pwrite64;

        #[cfg(not(target_os = "android"))]
        unsafe fn cvt_pwrite64(
            fd: c_int,
            buf: *const c_void,
            count: usize,
            offset: i64,
        ) -> io::Result<isize> {
            #[cfg(not(target_os = "linux"))]
            use netc::pwrite as pwrite64;
            #[cfg(target_os = "linux")]
            use netc::pwrite64;
            cvt(pwrite64(fd, buf, count, offset))
        }

        unsafe {
            cvt_pwrite64(
                self.fd,
                buf.as_ptr() as *const c_void,
                cmp::min(buf.len(), READ_LIMIT),
                offset as i64,
            )
            .map(|n| n as usize)
        }
    }

    #[cfg(target_os = "linux")]
    pub fn get_cloexec(&self) -> io::Result<bool> {
        unsafe { Ok((cvt(netc::fcntl(self.fd, netc::F_GETFD))? & netc::FD_CLOEXEC) != 0) }
    }

    // Setting `FD_CLOEXEC` is not supported on FreeRTOS
    // since there is no `exec` functionality.
    #[cfg(target_os = "freertos")]
    pub fn set_cloexec(&self) -> io::Result<()> {
        Ok(())
    }

    #[cfg(not(any(
        target_env = "newlib",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "l4re",
        target_os = "linux",
        target_os = "haiku",
        target_os = "redox",
        target_os = "vxworks"
    )))]
    pub fn set_cloexec(&self) -> io::Result<()> {
        unsafe {
            cvt(netc::ioctl(self.fd, netc::FIOCLEX))?;
            Ok(())
        }
    }
    #[cfg(any(
        all(target_env = "newlib", not(target_os = "freertos")),
        target_os = "solaris",
        target_os = "illumos",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "l4re",
        target_os = "linux",
        target_os = "haiku",
        target_os = "redox",
        target_os = "vxworks"
    ))]
    pub fn set_cloexec(&self) -> io::Result<()> {
        unsafe {
            let previous = cvt(netc::fcntl(self.fd, netc::F_GETFD))?;
            let new = previous | netc::FD_CLOEXEC;
            if new != previous {
                cvt(netc::fcntl(self.fd, netc::F_SETFD, new))?;
            }
            Ok(())
        }
    }

    #[cfg(target_os = "linux")]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        unsafe {
            let v = nonblocking as c_int;
            cvt(netc::ioctl(self.fd, netc::FIONBIO, &v))?;
            Ok(())
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        unsafe {
            let previous = cvt(netc::fcntl(self.fd, netc::F_GETFL))?;
            let new = if nonblocking {
                previous | netc::O_NONBLOCK
            } else {
                previous & !netc::O_NONBLOCK
            };
            if new != previous {
                cvt(netc::fcntl(self.fd, netc::F_SETFL, new))?;
            }
            Ok(())
        }
    }

    pub fn duplicate(&self) -> io::Result<NetFileDesc> {
        // We want to atomically duplicate this file descriptor and set the
        // CLOEXEC flag, and currently that's done via F_DUPFD_CLOEXEC. This
        // is a POSIX flag that was added to Linux in 2.6.24.
        let fd = cvt(unsafe { netc::fcntl(self.raw(), netc::F_DUPFD_CLOEXEC, 0) })?;
        Ok(NetFileDesc::new(fd))
    }
}

impl<'a> Read for &'a NetFileDesc {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }

    #[inline]
    unsafe fn initializer(&self) -> Initializer {
        Initializer::nop()
    }
}

impl AsInner<c_int> for NetFileDesc {
    fn as_inner(&self) -> &c_int {
        &self.fd
    }
}

impl Drop for NetFileDesc {
    fn drop(&mut self) {
        // Note that errors are ignored when closing a file descriptor. The
        // reason for this is that if an error occurs we don't actually know if
        // the file descriptor was closed or not, and if we retried (for
        // something like EINTR), we might close another valid file descriptor
        // opened after we closed ours.
        let _ = unsafe { netc::close(self.fd) };
    }
}
