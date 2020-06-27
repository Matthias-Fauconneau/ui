pub trait Take<'t, T> { fn take<'s>(&'s mut self, n: usize) -> &'t [T]; }
impl<'t, T> Take<'t, T> for &'t [T] {
    fn take<'s>(&'s mut self, mid: usize) -> &'t [T] {
        let (consumed, remaining) = std::mem::replace(self, &[]).split_at(mid);
        *self = remaining;
        consumed
    }
}
pub trait TakeMut<'t, T> { fn take_mut<'s>(&'s mut self, n: usize) -> &'t mut [T]; }
impl<'t, T> TakeMut<'t, T> for &'t mut [T] {
    fn take_mut<'s>(&'s mut self, mid: usize) -> &'t mut [T] {
        let (consumed, remaining) = std::mem::replace(self, &mut []).split_at_mut(mid);
        *self = remaining;
        consumed
    }
}

/// # Safety
///
/// T should be a basic type (i.e valid when casted from any data)
pub unsafe fn cast_mut<T>(slice: &mut [u8]) -> &mut [T] {
    std::slice::from_raw_parts_mut(slice.as_mut_ptr() as *mut T, slice.len() / std::mem::size_of::<T>())
}
