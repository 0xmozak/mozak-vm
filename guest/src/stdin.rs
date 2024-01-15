use std::io;
use std::io::Read;

pub struct MozakIo<'a> {
    pub stdin: Box<dyn Read + 'a>,
    #[cfg(not(target_os = "zkvm"))]
    pub file: String,
}

pub struct MozakIoPrivate<'a>(pub MozakIo<'a>);
pub struct MozakIoPublic<'a>(pub MozakIo<'a>);

#[cfg(not(target_os = "zkvm"))]
macro_rules! native_io_impl {
    ($s: ident) => {
        impl<'a> std::ops::Deref for $s<'a> {
            type Target = MozakIo<'a>;

            fn deref(&self) -> &Self::Target { &self.0 }
        }

        impl<'a> std::ops::DerefMut for $s<'a> {
            fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
        }
    };
}

#[cfg(not(target_os = "zkvm"))]
native_io_impl!(MozakIoPublic);
#[cfg(not(target_os = "zkvm"))]
native_io_impl!(MozakIoPrivate);

#[cfg(not(target_os = "zkvm"))]
impl<'a> Read for MozakIo<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        {
            let n_bytes = self.stdin.read(buf).expect("read should not fail");
            // open I/O log file in append mode.
            use std::io::Write;
            let mut out = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(self.file.as_str())
                .expect("cannot open file");
            out.write(buf).expect("write failed");
            Ok(n_bytes)
        }
    }
}

#[cfg(target_os = "zkvm")]
impl<'a> Read for MozakIoPrivate<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        {
            mozak_system::system::syscall_ioread_private(buf.as_mut_ptr(), buf.len());
            Ok(buf.len())
        }
    }
}

#[cfg(target_os = "zkvm")]
impl<'a> Read for MozakIoPublic<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        {
            mozak_system::system::syscall_ioread_public(buf.as_mut_ptr(), buf.len());
            Ok(buf.len())
        }
    }
}
