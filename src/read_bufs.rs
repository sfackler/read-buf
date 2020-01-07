use std::io::IoSliceMut;
use std::mem;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

/// A wrapper over a set of incrementally-initialized byte slices.
pub struct ReadBufs<'a> {
    bufs: &'a mut [MaybeUninitIoSliceMut<'a>],
    initialized: usize,
}

impl<'a> ReadBufs<'a> {
    /// Creates a new `ReadBufs` from a set of fully initialized slices.
    #[inline]
    pub fn new(bufs: &'a mut [IoSliceMut<'a>]) -> ReadBufs<'a> {
        ReadBufs {
            initialized: bufs.iter().map(|b| b.len()).sum(),
            bufs: unsafe { mem::transmute(bufs) },
        }
    }

    /// Creates a new `ReadBufs` from a set of fully uninitialized slices.
    ///
    /// Use `assume_initialized` if part of the slices are known to be already initialized.
    #[inline]
    pub fn new_uninit(bufs: &'a mut [MaybeUninitIoSliceMut<'a>]) -> ReadBufs<'a> {
        ReadBufs {
            bufs,
            initialized: 0,
        }
    }

    /// Returns the number of bytes at the beginning of the set of slices that are known to be initialized.
    #[inline]
    pub fn initialized(&self) -> usize {
        self.initialized
    }

    /// Asserts that the first `n` bytes at the beginning of the slices are initialized.
    ///
    /// `ReadBufs` assumes that bytes are never "de-initialized", so this method does nothing when called with fewer
    /// bytes than are already known to be initialized.
    #[inline]
    pub unsafe fn assume_initialized(&mut self, initialized: usize) {
        self.initialized = usize::max(self.initialized, initialized);
    }

    /// Returns a mutable reference to the entire set as maybe-uninitialized slices.
    ///
    /// # Safety
    ///
    /// The caller must not "de-initialize" bytes that are already known to have been initialized.
    #[inline]
    pub unsafe fn as_uninit(&mut self) -> &mut [MaybeUninitIoSliceMut<'a>] {
        self.bufs
    }

    /// Returns mutable references to the initialized and uninitialized portions of the slice.
    ///
    /// If a slices is partially-initialized, it will be included in the uninitialized portion.
    ///
    /// The two parts are guaranteed to cover the entire reange of the inner slices, and be directly contiguous.
    #[inline]
    pub fn as_slices(&mut self) -> (&mut [IoSliceMut<'a>], &mut [MaybeUninitIoSliceMut<'a>]) {
        let mut remaining = self.initialized;
        let split = self.bufs.iter().position(|b| {
            if remaining < b.len() {
                true
            } else {
                remaining -= b.len();
                false
            }
        });

        let split = match split {
            Some(split) => split,
            None if remaining == 0 => self.bufs.len(),
            None => panic!("invalid initialized state"),
        };

        let (head, tail) = self.bufs.split_at_mut(split);
        (unsafe { mem::transmute(head) }, tail)
    }

    /// Returns mutable references to the entire set of slices, initializing them as necessary.
    ///
    /// Since `ReadBufs` tracks the initialization state of the slice, this may be expensive the first time it is
    /// called, but is cheap after that.
    #[inline]
    pub fn as_init(&mut self) -> &mut [IoSliceMut<'a>] {
        self.as_init_to(self.bufs.iter().map(|buf| buf.len()).sum())
    }

    /// Returns mutable references to the first set of slices containing `len` bytes, initializing them as necessary.
    ///
    /// The number of bytes initialized and returned can be greater than `len`, as the method rounds up to the next
    /// slice boundary.
    ///
    /// # Panics
    ///
    /// Panics if the slices do not collectively have `len` elements.
    #[inline]
    pub fn as_init_to(&mut self, len: usize) -> &mut [IoSliceMut<'a>] {
        let mut seen_len = 0;
        let initialized = self.initialized;
        let cutoff = self
            .bufs
            .iter_mut()
            .position(|buf| {
                if seen_len + buf.len() > initialized {
                    for b in &mut buf[initialized.saturating_sub(len)..] {
                        *b = MaybeUninit::new(0);
                    }
                }
                seen_len += buf.len();

                if seen_len >= len {
                    true
                } else {
                    false
                }
            })
            .expect("invalid len");

        unsafe {
            self.assume_initialized(seen_len);

            mem::transmute(&mut self.bufs[..=cutoff])
        }
    }
}

/// A possibly-uninitialized version of `IoSliceMut`.
///
/// It is guaranteed to have exactly the same layout and ABI as `IoSliceMut`.
#[repr(transparent)]
pub struct MaybeUninitIoSliceMut<'a>(IoSliceMut<'a>);

impl<'a> MaybeUninitIoSliceMut<'a> {
    /// Creates a new `MaybeUninitIoSliceMut` from a slice of maybe-uninitialized bytes.
    #[inline]
    pub fn new(buf: &'a mut [MaybeUninit<u8>]) -> MaybeUninitIoSliceMut<'a> {
        unsafe { MaybeUninitIoSliceMut(IoSliceMut::new(mem::transmute(buf))) }
    }
}

impl Deref for MaybeUninitIoSliceMut<'_> {
    type Target = [MaybeUninit<u8>];

    #[inline]
    fn deref(&self) -> &[MaybeUninit<u8>] {
        unsafe { mem::transmute(&*self.0) }
    }
}

impl DerefMut for MaybeUninitIoSliceMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        unsafe { mem::transmute(&mut *self.0) }
    }
}

#[cfg(test)]
mod test {
    use crate::{MaybeUninitIoSliceMut, ReadBufs};
    use std::io::IoSliceMut;
    use std::mem::MaybeUninit;

    #[test]
    fn from_init() {
        let mut buf1 = [1; 5];
        let mut buf2 = [2; 0];
        let mut buf3 = [3; 3];
        let mut bufs = [
            IoSliceMut::new(&mut buf1),
            IoSliceMut::new(&mut buf2),
            IoSliceMut::new(&mut buf3),
        ];
        let mut bufs = ReadBufs::new(&mut bufs);

        assert_eq!(bufs.initialized(), 8);

        let (head, tail) = bufs.as_slices();
        assert_eq!(head.len(), 3);
        assert_eq!(tail.len(), 0);

        let init = bufs.as_init();
        assert_eq!(init.len(), 3);
        assert_eq!(&*init[0], &[1; 5][..]);
        assert_eq!(&*init[1], &[2; 0][..]);
        assert_eq!(&*init[2], &[3; 3][..]);
    }

    #[test]
    fn from_uninit() {
        let mut buf1 = [MaybeUninit::new(1); 5];
        let mut buf2 = [MaybeUninit::new(2); 0];
        let mut buf3 = [MaybeUninit::new(3); 3];
        let mut bufs = [
            MaybeUninitIoSliceMut::new(&mut buf1),
            MaybeUninitIoSliceMut::new(&mut buf2),
            MaybeUninitIoSliceMut::new(&mut buf3),
        ];
        let mut bufs = ReadBufs::new_uninit(&mut bufs);

        assert_eq!(bufs.initialized(), 0);

        let (head, tail) = bufs.as_slices();
        assert_eq!(head.len(), 0);
        assert_eq!(tail.len(), 3);

        let partial_init = bufs.as_init_to(1);
        assert_eq!(partial_init.len(), 1);
        assert_eq!(&*partial_init[0], &[0; 5][..]);
        partial_init[0].copy_from_slice(&[4; 5]);

        assert_eq!(bufs.initialized(), 5);
    }
}
