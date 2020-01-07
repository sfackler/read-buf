use std::mem::MaybeUninit;
use std::slice;

pub trait VecExt<T> {
    /// Returns a shared reference to the vector's spare capacity as a slice of maybe-uninitialized values.
    fn spare_capacity(&self) -> &[MaybeUninit<T>];

    /// Returns a mutable reference to the vector's spare capacity as a slice of maybe-uninitialized values.
    fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<T>];
}

impl<T> VecExt<T> for Vec<T> {
    #[inline]
    fn spare_capacity(&self) -> &[MaybeUninit<T>] {
        unsafe {
            slice::from_raw_parts(
                self.as_ptr().add(self.len()).cast::<MaybeUninit<T>>(),
                self.capacity() - self.len(),
            )
        }
    }

    #[inline]
    fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<T>] {
        unsafe {
            slice::from_raw_parts_mut(
                self.as_mut_ptr().add(self.len()).cast::<MaybeUninit<T>>(),
                self.capacity() - self.len(),
            )
        }
    }
}
