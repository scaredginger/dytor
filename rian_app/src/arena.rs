use std::{alloc::Layout, collections::HashMap, path::Path, sync::Arc};

use rian::serde_yaml;
use serde::Deserialize;

pub fn compute_space_bound<'a>(layouts: impl IntoIterator<Item = Layout>) -> usize {
    let mut res: usize = 0;
    let mut known_align: usize = 1;
    let mut curr_offset: usize = 0;

    for layout in layouts {
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

#[cfg(test)]
mod tests {
    use std::alloc::Layout;

    use super::compute_space_bound;

    #[test]
    fn space_bound1() {
        let res = compute_space_bound(
            [(4, 1), (2, 2), (2, 2)].map(|(s, a)| Layout::from_size_align(s, a).unwrap()),
        );
        assert_eq!(res, 9);
    }

    #[test]
    fn space_bound2() {
        let res = compute_space_bound(
            [(4, 4), (8, 8), (4, 4)].map(|(s, a)| Layout::from_size_align(s, a).unwrap()),
        );
        assert_eq!(res, 23);
    }
}
