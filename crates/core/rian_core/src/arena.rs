use std::{alloc::Layout, ptr::NonNull};

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub(crate) struct Offset(pub(crate) u32);

pub(crate) struct Arena {
    pub(crate) data: NonNull<u8>,
    pub(crate) capacity: usize,
}

// safety: Arena can be sent as long as nothing has been constructed in it
unsafe impl Send for Arena {}

#[derive(Copy, Clone)]
pub(crate) struct ArenaSlice(isize, usize);

#[derive(Default)]
pub(crate) struct ArenaBuilder {
    layouts: Vec<Layout>,
}

fn compute_space_required(layouts: &[Layout]) -> usize {
    let mut res: usize = 0;
    let mut known_align: usize = 1;
    let mut curr_offset: usize = 0;

    for &layout in layouts {
        let size = layout.size();
        let align = layout.align();
        if align > known_align {
            res += align - known_align + (known_align - curr_offset) % known_align;
            known_align = align;
            curr_offset = 0;
        } else {
            let padding = align - ((curr_offset + align - 1) % align) - 1;
            res += padding;
            curr_offset = (curr_offset + padding) % known_align;
        }
        res += size;
        curr_offset = (curr_offset + size) % known_align;
    }
    res
}

fn get_offsets(mut start: *mut u8, layouts: &[Layout]) -> Vec<Offset> {
    let mut res = Vec::with_capacity(layouts.len());
    for layout in layouts {
        let addr = start.wrapping_add(start.align_offset(layout.align()));
        let offset = unsafe { addr.offset_from(start) }.try_into().unwrap();
        res.push(Offset(offset));
        start = addr.wrapping_add(layout.size());
    }
    res
}

impl Arena {
    pub(crate) fn at_offset(&mut self, offset: usize, layout: Layout) -> &mut [u8] {
        let ptr = self.data.as_ptr().wrapping_add(offset);
        assert_eq!(ptr.align_offset(layout.align()), 0);
        assert!(self.data.as_ptr().wrapping_add(self.capacity) > ptr.wrapping_add(layout.size()));
        unsafe { std::slice::from_raw_parts_mut(ptr, layout.size()) }
    }

    pub(crate) unsafe fn at_offset_unchecked(
        &mut self,
        offset: usize,
        layout: Layout,
    ) -> &mut [u8] {
        let ptr = self.data.as_ptr().wrapping_add(offset);
        debug_assert_eq!(ptr.align_offset(layout.align()), 0);
        debug_assert!(
            self.data.as_ptr().wrapping_add(self.capacity) > ptr.wrapping_add(layout.size())
        );
        unsafe { std::slice::from_raw_parts_mut(ptr, layout.size()) }
    }

    pub(crate) fn from_layouts(layouts: &[Layout]) -> (Arena, Vec<Offset>) {
        let capacity = compute_space_required(layouts);
        let ptr = unsafe { std::alloc::alloc(Layout::from_size_align(capacity, 1).unwrap()) };
        let arena = Arena {
            data: NonNull::new(ptr).unwrap(),
            capacity,
        };
        (arena, get_offsets(ptr, layouts))
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        let ptr = self.data.as_ptr();
        let layout = Layout::from_size_align(self.capacity, 1).unwrap();
        unsafe { std::alloc::dealloc(ptr, layout) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_bound1() {
        let space = compute_space_required(
            &[(4, 1), (2, 2), (2, 2)].map(|(s, a)| Layout::from_size_align(s, a).unwrap()),
        );
        assert_eq!(space, 9);
    }

    #[test]
    fn space_bound2() {
        let space = compute_space_required(
            &[(4, 4), (8, 8), (4, 4)].map(|(s, a)| Layout::from_size_align(s, a).unwrap()),
        );
        assert_eq!(space, 23);
    }
}
