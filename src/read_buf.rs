use std::mem;
use std::mem::MaybeUninit;

/// A wrapper over a slice of incrementally-initialized bytes.
pub struct ReadBuf<'a> {
    buf: &'a mut [MaybeUninit<u8>],
    initialized: usize,
}

impl<'a> ReadBuf<'a> {
    /// Creates a new `ReadBuf` from a fully initialized slice.
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> ReadBuf<'a> {
        ReadBuf {
            initialized: buf.len(),
            buf: unsafe { mem::transmute(buf) },
        }
    }

    /// Creates a new `ReadBuf` from a fully uninitialized slice.
    ///
    /// Use `assume_initialized` if part of the slice is known to be already initialized.
    #[inline]
    pub fn new_uninit(buf: &'a mut [MaybeUninit<u8>]) -> ReadBuf<'a> {
        ReadBuf {
            buf,
            initialized: 0,
        }
    }

    /// Returns the size of the slice.
    #[inline]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns the number of bytes at the beginning of the slice that are known to be initialized.
    #[inline]
    pub fn initialized(&self) -> usize {
        self.initialized
    }

    /// Asserts that the first `n` bytes at the beginning of the slice are initialized.
    ///
    /// `ReadBuf` assumes that bytes are never "de-initialized", so this method does nothing when called with fewer
    /// bytes than are already known to be initialized.
    ///
    /// # Safety
    ///
    /// The caller must have already initialized the first `n` bytes of the slice.
    #[inline]
    pub unsafe fn assume_initialized(&mut self, n: usize) {
        self.initialized = usize::max(self.initialized, n);
    }

    /// Returns a mutable reference to the entire slice as maybe-uninitialized values.
    ///
    /// # Safety
    ///
    /// The caller must not "de-initialize" bytes that are already known to have been initialized.
    #[inline]
    pub unsafe fn as_uninit(&mut self) -> &mut [MaybeUninit<u8>] {
        self.buf
    }

    /// Returns mutable references to the initialized and uninitialized portions of the slice.
    ///
    /// The two parts are guaranteed to cover the entire range of the inner slice, and be directly contiguous.
    #[inline]
    pub fn as_slices(&mut self) -> (&mut [u8], &mut [MaybeUninit<u8>]) {
        let (head, tail) = self.buf.split_at_mut(self.initialized);
        (unsafe { cast_init(head) }, tail)
    }

    /// Returns a mutable reference to the entire slice, initializing it as necessary.
    ///
    /// Since `ReadBuf` tracks the initialization state of the slice, this may be expensive the first time it is called
    /// but is "free" after that.
    #[inline]
    pub fn as_init(&mut self) -> &mut [u8] {
        self.as_init_to(self.buf.len())
    }

    /// Returns a mutable reference to the first `len` bytes of the slice, initializing it as necessary.
    ///
    /// Since `ReadBuf` tracks the initialization state of the slice, this may be expensive the first time it is called
    /// but is "free" after that.
    ///
    /// # Panics
    ///
    /// Panics if the slice does not have `len` elements.
    #[inline]
    pub fn as_init_to(&mut self, len: usize) -> &mut [u8] {
        if len > self.initialized {
            for b in &mut self.buf[self.initialized..len] {
                *b = MaybeUninit::new(0);
            }
            self.initialized = len;
        }
        unsafe { cast_init(&mut self.buf[..len]) }
    }
}

#[inline]
unsafe fn cast_init(buf: &mut [MaybeUninit<u8>]) -> &mut [u8] {
    mem::transmute(buf)
}

#[cfg(test)]
mod test {
    use crate::ReadBuf;
    use std::mem::MaybeUninit;

    #[test]
    fn from_init() {
        let mut buf = [1; 10];
        let mut buf = ReadBuf::new(&mut buf);

        assert_eq!(buf.initialized(), 10);

        let (head, tail) = buf.as_slices();
        assert_eq!(head, &[1; 10][..]);
        assert_eq!(tail.len(), 0);

        let init = buf.as_init();
        assert_eq!(init, &[1; 10][..]);
    }

    #[test]
    fn from_uninit() {
        let mut buf = [MaybeUninit::new(1); 10];
        let mut buf = ReadBuf::new_uninit(&mut buf);

        assert_eq!(buf.initialized(), 0);

        let (head, tail) = buf.as_slices();
        assert_eq!(head, &mut []);
        assert_eq!(tail.len(), 10);

        let partial_init = buf.as_init_to(5);
        assert_eq!(partial_init, &mut [0; 5][..]);
        partial_init.copy_from_slice(&[2; 5]);

        assert_eq!(buf.initialized(), 5);

        let (head, tail) = buf.as_slices();
        assert_eq!(head, &mut [2; 5][..]);
        assert_eq!(tail.len(), 5);

        let init = buf.as_init();
        assert_eq!(init, &mut [2, 2, 2, 2, 2, 0, 0, 0, 0, 0][..]);

        assert_eq!(buf.initialized(), 10);
    }
}
