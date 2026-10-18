#[cfg(feature = "alloc")]
mod in_memory {
    use crate::{IoError, PollReader, Reader, SwapOffsets};

    pub enum Bytes<'a, A: alloc::alloc::Allocator = alloc::alloc::Global> {
        Borrowed(&'a [u8]),
        Owned(alloc::vec::Vec<u8, A>),
    }

    impl<'a, A: alloc::alloc::Allocator> Bytes<'a, A> {
        fn bytes(&self) -> &[u8] {
            match self {
                Self::Borrowed(b) => b,
                Self::Owned(o) => o.as_ref(),
            }
        }
    }

    pub struct InMemoryReader<'a, A: alloc::alloc::Allocator = alloc::alloc::Global> {
        bytes: Bytes<'a, A>,
        offset: usize,
    }

    impl InMemoryReader<'_> {
        #[cfg(feature = "std")]
        pub fn from_path<P: AsRef<std::path::Path>>(p: P) -> Result<Self, IoError> {
            let bytes = std::fs::read(p)?;
            Ok(Self {
                bytes: Bytes::Owned(bytes),
                offset: 0,
            })
        }

        pub fn from_bytes<'a>(bytes: &'a [u8]) -> InMemoryReader<'a> {
            InMemoryReader {
                offset: 0,
                bytes: Bytes::Borrowed(bytes),
            }
        }

        pub fn from_buf<A: alloc::alloc::Allocator>(
            buf: alloc::vec::Vec<u8, A>,
        ) -> InMemoryReader<'static, A> {
            InMemoryReader {
                offset: 0,
                bytes: Bytes::Owned(buf),
            }
        }
    }

    impl SwapOffsets<usize> for InMemoryReader<'_> {
        fn swap_offsets(&mut self, offset: &mut usize) {
            core::mem::swap(&mut self.offset, offset)
        }
    }

    impl Reader<usize> for InMemoryReader<'_> {
        fn remaining_size(&self) -> usize {
            self.bytes
                .bytes()
                .len()
                .checked_sub(self.offset)
                .unwrap_or_default()
        }

        fn offset(&self) -> usize {
            core::cmp::min(self.offset, self.bytes.bytes().len())
        }

        fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
            let remaining = Reader::remaining_size(self);
            if remaining < bytes.len() {
                return Err(IoError::UnexpectedEndOfData);
            }
            bytes.copy_from_slice(&self.bytes.bytes()[self.offset..self.offset + bytes.len()]);
            self.offset += bytes.len();
            Ok(())
        }

        fn read_cstr(&mut self) -> Result<usize, IoError> {
            let bytes = &self.bytes.bytes()[self.offset..];
            let len = bytes
                .iter()
                .position(|&byte| byte == b'\0')
                .ok_or(IoError::UnexpectedEndOfData)?;
            self.offset += len + 1;
            Ok(len)
        }

        fn seek(&mut self, amt: usize) -> Result<(), IoError> {
            let remaining = Reader::remaining_size(self);
            if amt > remaining {
                return Err(IoError::UnexpectedEndOfData);
            }
            self.offset += amt;
            Ok(())
        }

        #[cfg(feature = "alloc")]
        fn read_extent_in<A: alloc::alloc::Allocator>(
            &self,
            extent: crate::Extent<usize, usize>,
            alloc: A,
        ) -> Result<alloc::boxed::Box<[u8], A>, crate::ExtentError> {
            if extent.offset + extent.len > self.bytes.bytes().len() {
                return Err(crate::ExtentError::Io(IoError::UnexpectedEndOfData));
            }

            let bytes = &self.bytes.bytes()[extent.offset..(extent.offset + extent.len)];

            let mut v = alloc::vec::Vec::new_in(alloc);
            v.try_reserve_exact(bytes.len())
                .map_err(|_| alloc::alloc::AllocError)?;
            v.extend_from_slice(bytes);
            Ok(v.into_boxed_slice())
        }
    }

    impl PollReader<usize> for InMemoryReader<'_> {
        fn poll_read(
            mut self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
            buf: &mut [u8],
        ) -> core::task::Poll<Result<(), IoError>> {
            let remaining = PollReader::remaining_size(&*self);
            if remaining < buf.len() {
                return core::task::Poll::Ready(Err(IoError::UnexpectedEndOfData));
            }

            buf.copy_from_slice(&self.bytes.bytes()[self.offset..self.offset + buf.len()]);
            self.offset += buf.len();

            core::task::Poll::Ready(Ok(()))
        }

        fn poll_read_cstr(
            mut self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<usize, IoError>> {
            let bytes = &self.bytes.bytes()[self.offset..];
            let len = bytes
                .iter()
                .position(|&byte| byte == b'\0')
                .ok_or(IoError::UnexpectedEndOfData)?;
            self.offset += len + 1;
            core::task::Poll::Ready(Ok(len))
        }

        fn seek_start(
            mut self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
            amt: usize,
        ) -> Result<(), IoError> {
            let remaining = PollReader::remaining_size(&*self);
            if amt > remaining {
                return Err(IoError::UnexpectedEndOfData);
            }

            self.offset += amt;
            Ok(())
        }

        fn poll_seek_complete(
            self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), IoError>> {
            core::task::Poll::Ready(Ok(()))
        }

        fn offset(&self) -> usize {
            core::cmp::min(self.offset, self.bytes.bytes().len())
        }
        fn remaining_size(&self) -> usize {
            self.bytes
                .bytes()
                .len()
                .checked_sub(self.offset)
                .unwrap_or_default()
        }

        #[cfg(feature = "alloc")]
        fn poll_read_extent<A: alloc::alloc::Allocator>(
            self: core::pin::Pin<&mut Self>,
            extent: &crate::Extent<usize, usize>,
            buf: &mut alloc::vec::Vec<u8, A>,
            _: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), crate::ExtentError>> {
            if extent.offset + extent.len > self.bytes.bytes().len() {
                return core::task::Poll::Ready(Err(crate::ExtentError::Io(
                    IoError::UnexpectedEndOfData,
                )));
            }

            let bytes = &self.bytes.bytes()[extent.offset..(extent.offset + extent.len)];
            buf.extend_from_slice(bytes);
            core::task::Poll::Ready(Ok(()))
        }
    }
}

#[cfg(feature = "alloc")]
pub use in_memory::*;

#[cfg(all(feature = "std", unix))]
mod file_backed {
    use crate::{IoError, PollReader, Reader, SwapOffsets};

    pub struct FileReader<F, const N: usize> {
        fd: F,
        buffer: [u8; N],
        file_offset: u64,
        buffer_offset: Option<usize>,
        file_len: u64,
    }

    impl<F: core::borrow::Borrow<std::fs::File>, const N: usize> FileReader<F, N> {
        pub fn new(fd: F, len: u64) -> Self {
            Self {
                fd,
                buffer: [0; N],
                file_offset: 0,
                buffer_offset: None,
                file_len: len,
            }
        }
    }

    impl<F: Clone + core::borrow::Borrow<std::fs::File>, const N: usize> Clone for FileReader<F, N> {
        fn clone(&self) -> Self {
            Self::new(self.fd.clone(), self.file_len)
        }
    }

    impl<const N: usize> FileReader<std::fs::File, N> {
        pub fn from_path<P: core::convert::AsRef<std::path::Path>>(
            path: P,
        ) -> Result<Self, crate::IoError> {
            let fd = std::fs::File::open(path)?;
            let len = fd.metadata()?.len();
            Ok(Self::new(fd, len))
        }
    }

    impl<const N: usize> FileReader<std::sync::Arc<std::fs::File>, N> {
        pub fn from_path_arced<P: core::convert::AsRef<std::path::Path>>(
            path: P,
        ) -> Result<Self, crate::IoError> {
            let fd = std::fs::File::open(path)?;
            let len = fd.metadata()?.len();
            Ok(Self::new(std::sync::Arc::new(fd), len))
        }
    }

    impl<F, const N: usize> SwapOffsets<u64> for FileReader<F, N> {
        fn swap_offsets(&mut self, offset: &mut u64) {
            // remove the offset so that the buffer is invalidated -> new io
            if let Some(offset) = core::mem::take(&mut self.buffer_offset) {
                self.file_offset += offset as u64;
            }
            core::mem::swap(&mut self.file_offset, offset)
        }
    }

    impl<F: core::borrow::Borrow<std::fs::File>, const N: usize> Reader<u64> for FileReader<F, N> {
        fn remaining_size(&self) -> u64 {
            self.file_len
                .checked_sub(
                    self.file_offset + self.buffer_offset.map(|ofs| ofs as u64).unwrap_or_default(),
                )
                .unwrap_or_default()
        }

        fn offset(&self) -> u64 {
            core::cmp::min(
                self.file_offset + self.buffer_offset.map(|ofs| ofs as u64).unwrap_or_default(),
                self.file_len,
            )
        }

        fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
            let remaining = Reader::remaining_size(self);
            if remaining < bytes.len() as u64 {
                return Err(IoError::UnexpectedEndOfData);
            }

            assert!(
                bytes.len() <= N,
                "more than N bytes read at a time is unsupported at the moment"
            );

            match self.buffer_offset.as_mut() {
                // in cache
                Some(offset) if N.checked_sub(*offset).unwrap_or_default() > bytes.len() => {
                    bytes.copy_from_slice(&self.buffer[*offset..*offset + bytes.len()]);
                    *offset += bytes.len();
                }
                _ => {
                    if let Some(offset) = core::mem::take(&mut self.buffer_offset) {
                        self.file_offset += offset as u64;
                    }
                    let read_buf = &mut self.buffer[..core::cmp::min(remaining as usize, N)];
                    let fd = self.fd.borrow();
                    use std::os::unix::fs::FileExt;
                    fd.read_exact_at(read_buf, self.file_offset)?;

                    bytes.copy_from_slice(&self.buffer[..bytes.len()]);
                    self.buffer_offset = Some(bytes.len());
                }
            }
            Ok(())
        }

        fn read_cstr(&mut self) -> Result<u64, IoError> {
            let mut len = 0;
            if let Some(offset) = self.buffer_offset.as_mut() {
                let bytes = &self.buffer[*offset..];

                if let Some(pos) = bytes.iter().position(|&byte| byte == b'\0') {
                    *offset += pos + 1;
                    return Ok(pos as u64);
                }
                len = bytes.len();
            }

            if let Some(offset) = core::mem::take(&mut self.buffer_offset) {
                self.file_offset += offset as u64;
            }

            loop {
                let remaining = Reader::remaining_size(self);
                let read_buf = &mut self.buffer[..core::cmp::min(remaining as usize, N)];

                use std::os::unix::fs::FileExt;
                let fd = self.fd.borrow();
                fd.read_exact_at(read_buf, self.file_offset)?;

                if let Some(pos) = read_buf.iter().position(|&byte| byte == b'\0') {
                    self.buffer_offset = Some(pos + 1);
                    return Ok((len + pos) as u64);
                }
                len += read_buf.len();
                self.file_offset += read_buf.len() as u64;
            }
        }

        fn seek(&mut self, amt: u64) -> Result<(), IoError> {
            let remaining = Reader::remaining_size(self);
            if amt > remaining {
                return Err(IoError::UnexpectedEndOfData);
            }

            match self.buffer_offset.as_mut() {
                Some(offset) if N.checked_sub(*offset).unwrap_or_default() as u64 > amt => {
                    *offset += amt as usize;
                }
                _ => {
                    if let Some(offset) = core::mem::take(&mut self.buffer_offset) {
                        self.file_offset += offset as u64;
                    }
                    self.file_offset += amt;
                }
            }
            Ok(())
        }

        #[cfg(feature = "alloc")]
        fn read_extent_in<A: alloc::alloc::Allocator>(
            &self,
            extent: crate::Extent<u64, u64>,
            alloc: A,
        ) -> Result<alloc::boxed::Box<[u8], A>, crate::ExtentError> {
            if extent.offset + extent.len > self.file_len {
                return Err(crate::ExtentError::Io(IoError::UnexpectedEndOfData));
            }

            let mut v = alloc::vec::Vec::new_in(alloc);
            let len: usize = extent
                .len
                .try_into()
                .map_err(|_| crate::ExtentError::IntegerConversion(crate::TryFromIntError))?;
            v.try_reserve_exact(len)
                .map_err(|_| alloc::alloc::AllocError)?;

            v.extend(core::iter::repeat(0).take(len));

            let fd = self.fd.borrow();
            use std::os::unix::fs::FileExt;
            fd.read_exact_at(v.as_mut_slice(), extent.offset)
                .map_err(|e| crate::IoError::Io(e))?;

            Ok(v.into_boxed_slice())
        }
    }

    pub struct AsyncFileReader<F, const N: usize> {
        fd: F,
        buffer: [u8; N],
        file_offset: u64,
        buffer_offset: Option<usize>,
        file_len: u64,
        io_task: Option<std::thread::JoinHandle<Result<[u8; N], IoError>>>,
        cstr_task: Option<std::thread::JoinHandle<Result<u64, IoError>>>,
        extent_task: Option<std::thread::JoinHandle<Result<Vec<u8>, IoError>>>,
        tasks_finished: std::sync::Arc<std::sync::atomic::AtomicU8>,
    }

    impl<F: core::borrow::Borrow<std::fs::File>, const N: usize> AsyncFileReader<F, N> {
        pub fn new(fd: F, len: u64) -> Self {
            Self {
                fd,
                buffer: [0; N],
                file_offset: 0,
                buffer_offset: None,
                file_len: len,
                io_task: None,
                cstr_task: None,
                extent_task: None,
                tasks_finished: std::sync::Arc::new(std::sync::atomic::AtomicU8::from(0)),
            }
        }
        const IO_TASK: u8 = 1 << 0;
        const CSTR_TASK: u8 = 1 << 1;
        const EXTENT_TASK: u8 = 1 << 2;
    }

    impl<F: Clone + core::borrow::Borrow<std::fs::File>, const N: usize> Clone
        for AsyncFileReader<F, N>
    {
        fn clone(&self) -> Self {
            Self::new(self.fd.clone(), self.file_len)
        }
    }

    impl<F, const N: usize> SwapOffsets<u64> for AsyncFileReader<F, N> {
        fn swap_offsets(&mut self, offset: &mut u64) {
            // remove the offset so that the buffer is invalidated -> new io
            if let Some(offset) = core::mem::take(&mut self.buffer_offset) {
                self.file_offset += offset as u64;
            }
            // drop any task if its pending for whatever reason
            self.io_task = None;
            self.cstr_task = None;
            self.extent_task = None;
            self.tasks_finished
                .store(0, std::sync::atomic::Ordering::SeqCst);
            core::mem::swap(&mut self.file_offset, offset)
        }
    }

    impl<const N: usize> PollReader<u64> for AsyncFileReader<std::sync::Arc<std::fs::File>, N> {
        fn poll_read(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
            buf: &mut [u8],
        ) -> core::task::Poll<Result<(), IoError>> {
            if self.io_task.is_some() {
                if self
                    .tasks_finished
                    .load(std::sync::atomic::Ordering::SeqCst)
                    & Self::IO_TASK
                    == 0
                {
                    return core::task::Poll::Pending;
                }
                // clear finished bit
                self.tasks_finished
                    .fetch_and(!Self::IO_TASK, std::sync::atomic::Ordering::SeqCst);

                let handle = core::mem::take(&mut self.io_task).unwrap();
                let bytes = handle.join().unwrap();
                match bytes {
                    Ok(bytes) => {
                        self.buffer = bytes;
                        buf.copy_from_slice(&self.buffer[..buf.len()]);
                        self.buffer_offset = Some(buf.len());
                        return core::task::Poll::Ready(Ok(()));
                    }
                    Err(e) => {
                        return core::task::Poll::Ready(Err(e));
                    }
                }
            }

            let remaining = PollReader::remaining_size(&*self);
            if remaining < buf.len() as u64 {
                return core::task::Poll::Ready(Err(IoError::UnexpectedEndOfData));
            }

            assert!(
                buf.len() <= N,
                "more than N bytes read at a time is unsupported at the moment"
            );

            match self.buffer_offset.as_mut() {
                // in cache
                Some(offset) if N.checked_sub(*offset).unwrap_or_default() > buf.len() => {
                    let ofs = *offset;
                    *offset += buf.len();
                    buf.copy_from_slice(&self.buffer[ofs..ofs + buf.len()]);
                }
                _ => {
                    if let Some(offset) = core::mem::take(&mut self.buffer_offset) {
                        self.file_offset += offset as u64;
                    }

                    let file_offset = self.file_offset;
                    let fd = self.fd.clone();
                    let waker = cx.waker().clone();
                    let tasks_finished = self.tasks_finished.clone();
                    self.io_task = Some(std::thread::spawn(move || {
                        let mut bytes = [0; N];
                        let read_buf = &mut bytes[..core::cmp::min(remaining as usize, N)];

                        use std::os::unix::fs::FileExt;
                        fd.read_exact_at(read_buf, file_offset)?;
                        tasks_finished.fetch_or(Self::IO_TASK, std::sync::atomic::Ordering::SeqCst);
                        waker.wake_by_ref();
                        Ok(bytes)
                    }));
                    return core::task::Poll::Pending;
                }
            }
            core::task::Poll::Ready(Ok(()))
        }

        fn poll_read_cstr(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<u64, IoError>> {
            if self.cstr_task.is_some() {
                if self
                    .tasks_finished
                    .load(std::sync::atomic::Ordering::SeqCst)
                    & Self::CSTR_TASK
                    == 0
                {
                    return core::task::Poll::Pending;
                }
                // clear bit
                self.tasks_finished
                    .fetch_and(!Self::CSTR_TASK, std::sync::atomic::Ordering::SeqCst);

                match core::mem::take(&mut self.cstr_task)
                    .unwrap()
                    .join()
                    .unwrap()
                {
                    Ok(len) => {
                        self.file_offset += len + 1;
                        return core::task::Poll::Ready(Ok(len));
                    }
                    Err(e) => return core::task::Poll::Ready(Err(e)),
                }
            }

            let this = &mut *self;
            if let Some(offset) = this.buffer_offset.as_mut() {
                let bytes = &this.buffer[*offset..];

                if let Some(pos) = bytes.iter().position(|&byte| byte == b'\0') {
                    *offset += pos + 1;
                    return core::task::Poll::Ready(Ok(pos as u64));
                }
            }

            if let Some(offset) = core::mem::take(&mut self.buffer_offset) {
                self.file_offset += offset as u64;
            }

            let remaining = PollReader::remaining_size(&*self);
            let file_offset = self.file_offset;
            let fd = self.fd.clone();
            let waker = cx.waker().clone();
            let tasks_finished = self.tasks_finished.clone();
            self.cstr_task = Some(std::thread::spawn(move || {
                let mut remaining = remaining;
                let mut file_offset = file_offset;
                let mut bytes = [0; N];
                let mut len = 0;
                loop {
                    let read_buf = &mut bytes[..core::cmp::min(remaining as usize, N)];

                    use std::os::unix::fs::FileExt;
                    fd.read_exact_at(read_buf, file_offset)?;

                    if let Some(pos) = read_buf.iter().position(|&byte| byte == b'\0') {
                        tasks_finished
                            .fetch_or(Self::CSTR_TASK, std::sync::atomic::Ordering::SeqCst);
                        waker.wake_by_ref();
                        return Ok(len + pos as u64);
                    }
                    len += N as u64;
                    file_offset += N as u64;
                    remaining = remaining.saturating_sub(N as u64);
                }
            }));
            core::task::Poll::Pending
        }

        fn seek_start(
            mut self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
            amt: u64,
        ) -> Result<(), IoError> {
            let remaining = PollReader::remaining_size(&*self);
            if amt > remaining {
                return Err(IoError::UnexpectedEndOfData);
            }

            match self.buffer_offset.as_mut() {
                Some(offset) if N.checked_sub(*offset).unwrap_or_default() as u64 > amt => {
                    *offset += amt as usize;
                }
                _ => {
                    if let Some(offset) = core::mem::take(&mut self.buffer_offset) {
                        self.file_offset += offset as u64;
                    }
                    self.file_offset += amt;
                }
            }
            Ok(())
        }

        fn poll_seek_complete(
            self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), IoError>> {
            core::task::Poll::Ready(Ok(()))
        }

        fn offset(&self) -> u64 {
            core::cmp::min(
                self.file_offset + self.buffer_offset.map(|ofs| ofs as u64).unwrap_or_default(),
                self.file_len,
            )
        }

        fn remaining_size(&self) -> u64 {
            self.file_len
                .checked_sub(
                    self.file_offset + self.buffer_offset.map(|ofs| ofs as u64).unwrap_or_default(),
                )
                .unwrap_or_default()
        }

        #[cfg(feature = "alloc")]
        fn poll_read_extent<A: alloc::alloc::Allocator>(
            mut self: core::pin::Pin<&mut Self>,
            extent: &crate::Extent<u64, u64>,
            buf: &mut alloc::vec::Vec<u8, A>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), crate::ExtentError>> {
            if self.extent_task.is_some() {
                if self
                    .tasks_finished
                    .load(std::sync::atomic::Ordering::SeqCst)
                    & Self::EXTENT_TASK
                    == 0
                {
                    return core::task::Poll::Pending;
                }
                // clear bit
                self.tasks_finished
                    .fetch_and(!Self::EXTENT_TASK, std::sync::atomic::Ordering::SeqCst);

                let bytes = core::mem::take(&mut self.extent_task)
                    .unwrap()
                    .join()
                    .unwrap()?;
                buf.extend_from_slice(&bytes);
                return core::task::Poll::Ready(Ok(()));
            }

            if extent.offset + extent.len > self.file_len {
                return core::task::Poll::Ready(Err(crate::ExtentError::Io(
                    IoError::UnexpectedEndOfData,
                )));
            }

            let extent = extent.clone();
            let waker = cx.waker().clone();
            let fd = self.fd.clone();
            let len = extent.len;
            let tasks_finished = self.tasks_finished.clone();

            self.extent_task = Some(std::thread::spawn(move || {
                let mut v = vec![0; len as usize];
                use std::os::unix::fs::FileExt;
                fd.read_exact_at(v.as_mut_slice(), extent.offset)
                    .map_err(|e| crate::IoError::Io(e))?;
                tasks_finished.fetch_or(Self::EXTENT_TASK, std::sync::atomic::Ordering::SeqCst);
                waker.wake_by_ref();
                Ok(v)
            }));
            core::task::Poll::Pending
        }
    }
}

#[cfg(all(feature = "std", unix))]
pub use file_backed::*;
