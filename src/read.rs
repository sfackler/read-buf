use crate::{ReadBuf, ReadBufs, VecExt};
use std::io::{self, Read, Write};
use std::mem::MaybeUninit;
use std::net::TcpStream;
use std::os::unix::io::AsRawFd;

pub trait Read2: Read {
    /// Pull some bytes from this source into the specified buffer, returning how many bytes were read.
    ///
    /// This is equivalent to the `read` method, except that it is passed a `ReadBuf` rather than `[u8]` to allow use
    /// with uninitialized buffers.
    ///
    /// The default implementation delegates to `read`.
    fn read_buf(&mut self, buf: &mut ReadBuf) -> io::Result<usize> {
        self.read(buf.as_init())
    }

    /// Like `read_buf`, except that it reads into a slice of buffers.
    ///
    /// This is equivalent to the `read_vectored` method, except that it is passed a `ReadBufs` rather than
    /// `[IoSliceMut]` to allow use with uninitialized buffers.
    ///
    /// The default implementation delegates to `read_vectored`.
    fn read_bufs(&mut self, bufs: &mut ReadBufs) -> io::Result<usize> {
        self.read_vectored(bufs.as_init())
    }

    /// Read the exact number of bytes required to fill `buf`.
    ///
    /// This is equivalent to the `read_exact` method, except that it is passed a `ReadBuf` rather than `[u8]` to allow
    /// use with uninitialized buffers.
    fn read_buf_exact(&mut self, buf: &mut ReadBuf) -> io::Result<()> {
        let mut base = 0;
        while buf.len() > base {
            let mut temp_buf = unsafe {
                let temp_init = buf
                    .initialized()
                    .checked_sub(base)
                    .expect("invalid initialized state");
                let mut temp_buf = ReadBuf::new_uninit(&mut buf.as_uninit()[base..]);
                temp_buf.assume_initialized(temp_init);
                temp_buf
            };
            let len = self.read_buf(&mut temp_buf)?;
            if len == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
            }

            let new_initialized = base + temp_buf.initialized();
            unsafe {
                buf.assume_initialized(new_initialized);
            }
            base += len;
        }

        Ok(())
    }

    /// Read all bytes until EOF in this source, placing them into `buf`.
    ///
    /// This is equivalent to `read_to_end`, except that it uses `read_buf` rather than `read`, allowing it to avoid
    /// initializing components of `buf` before filling them.
    fn read_to_end2(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let initial_len = buf.len();

        let mut initialized = 0;
        loop {
            if buf.len() == buf.capacity() {
                buf.reserve(32);
            }

            let mut read_buf = ReadBuf::new_uninit(buf.spare_capacity_mut());
            unsafe {
                read_buf.assume_initialized(initialized);
            }

            let nread = self.read_buf(&mut read_buf)?;
            if nread == 0 {
                return Ok(buf.len() - initial_len);
            }

            initialized = read_buf
                .initialized()
                .checked_sub(nread)
                .expect("invalid initialized state");
            let new_len = buf.len() + nread;
            unsafe {
                buf.set_len(new_len);
            }
        }
    }
}

impl Read2 for TcpStream {
    fn read_buf(&mut self, buf: &mut ReadBuf) -> io::Result<usize> {
        unsafe {
            let raw_buf = buf.as_uninit();
            let ret = libc::read(self.as_raw_fd(), raw_buf.as_mut_ptr().cast(), raw_buf.len());
            if ret < 0 {
                Err(io::Error::last_os_error())
            } else {
                let len = ret as usize;
                buf.assume_initialized(len);
                Ok(len)
            }
        }
    }

    fn read_bufs(&mut self, bufs: &mut ReadBufs) -> io::Result<usize> {
        unsafe {
            let raw_bufs = bufs.as_uninit();
            let ret = libc::readv(
                self.as_raw_fd(),
                raw_bufs.as_mut_ptr().cast(),
                raw_bufs.len() as i32,
            );
            if ret < 0 {
                Err(io::Error::last_os_error())
            } else {
                let len = ret as usize;
                bufs.assume_initialized(len);
                Ok(len)
            }
        }
    }
}

/// A reimplementation of `io::copy`, except that it uses `read_buf` to avoid initializing the stack buffer.
pub fn copy<R, W>(reader: &mut R, writer: &mut W) -> io::Result<u64>
where
    R: Read2,
    W: Write,
{
    let mut buf = [MaybeUninit::uninit(); 4096];
    let mut buf = ReadBuf::new_uninit(&mut buf);
    let mut len = 0;

    loop {
        let nread = reader.read_buf(&mut buf)?;
        if nread == 0 {
            return Ok(len);
        }
        len += nread as u64;
        writer.write_all(&buf.as_slices().0[..nread])?;
    }
}
