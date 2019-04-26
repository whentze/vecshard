/*!
Split Vecs in O(1) time.

You can split a [`Vec`] into two using [`Vec::split_off`](std::vec::Vec::split_off),
but since most allocators can't just go and split up an allocation, this needs to allocate space
for a second [`Vec`] and, even worse, copy the relevant elements over, which takes O(n) time.
You could also split it into slices using `Vec::split_at` or
`Vec::split_at_mut`, but this will not give you owned
data you can move around or move out of at will.

This crate provides a way to split a [`Vec`] into two owned [`VecShard`]s that
behave similar to Vecs that takes constant time.
The catch is that the [`VecShard`]s use reference counting to determine when the last of them is dropped.
Only then is the memory from the original [`Vec`] deallocated.
The individual items in the shards, however, are dropped as soon as the shard is dropped.

This functionality is provided through an extension trait for [`Vec`], [`ShardExt`](crate::ShardExt).

# Basic Example

```
use vecshard::ShardExt;

let animals = vec!["penguin", "owl", "toucan", "turtle", "spider", "mosquitto"];

// split the vec into 2 shards
let (cool_animals, uncool_animals) = animals.split_inplace_at(4);

// shards can be indexed as usual
assert_eq!(cool_animals[3], "turtle");
assert_eq!(uncool_animals[0], "spider");

// ..including with a range as index
assert_eq!(cool_animals[1..3], ["owl", "toucan"]);

// they deref into slices, so you can use them as such:
assert_eq!(cool_animals.len(), 4);
assert!(uncool_animals.ends_with(&["mosquitto"]));

// shards can also be split up again:
let (cool_birds, cool_reptiles) = cool_animals.split_inplace_at(3);
assert_eq!(*cool_birds, ["penguin", "owl", "toucan"]);
assert_eq!(*cool_reptiles, ["turtle"]);
```

# Conversion

Shards can be freely converted both [`From`](std::convert::From) and [`Into`](std::convert::Into) Vecs.
Note that the latter may need to allocate if there are other shards also using the shards allocation.

```
# use vecshard::{VecShard, ShardExt};

let vec = vec![1, 2, 3];
let shard = VecShard::from(vec);
let vec2 : Vec<_> = shard.into();
```

# Iteration

To iterate over a [`VecShard`], you have several choices.
[`VecShard<T>`](crate::VecShard) itself is a draining [`Iterator`] and returns owned `T` instances,
removing them from its own storage.
If you only need `&T` or `&mut T`, you can deref it to a slice and iterate over that.
Finally, if you need an owning [`Iterator`] but do not want to drain the shard,
you can [`clone`][std::clone::Clone::clone] the shard and iterate over that.

```
# use vecshard::{VecShard, ShardExt};
let mut shard = VecShard::from(vec!['y', 'e', 'e', 't']);

assert_eq!(Some('y'), shard.next());
assert_eq!(Some('e'), shard.next());

assert_eq!(*shard, ['e', 't']);
```

# Optional Features

This crate has zero dependencies by default, but if you want to serialize and deserialize `VecShard`,
you can enable the `serde` feature like this:

```toml
[dependencies.vecshard]
optional = true
version = "0.2.1"
```

[`VecShard`]: crate::VecShard
*/

use std::{
    borrow::{Borrow, BorrowMut},
    cmp::{Eq, PartialEq},
    fmt,
    hash::{Hash, Hasher},
    iter::FusedIterator,
    mem,
    ops::{Deref, DerefMut, Index, IndexMut},
    ptr,
    slice::{self, SliceIndex},
    sync::Arc,
};

pub mod error;
use crate::error::{CantMerge, WouldAlloc, WouldMove};

#[cfg(feature = "serde")]
mod serde_impl;

/// An extension trait for things that can be split into shards
///
/// For your convenience, this is implemented for both [`Vec`](std::vec::Vec) and
/// [`VecShard`](crate::VecShard), so you can split recursively:
///
/// ```
/// # use vecshard::ShardExt;
/// let drinks = vec!["heineken", "jupiler", "turmbräu", "orange juice", "champagne"];
///
/// let (beers, other_drinks) = drinks.split_inplace_at(3);
/// let (bad_beers, good_beers) = beers.split_inplace_at(2);
///
/// assert_eq!(*good_beers, ["turmbräu"]);
/// ```
pub trait ShardExt {
    type Shard;

    /// Split this array into two shards at the given index.
    /// This is an O(1) operation, as it keeps the underlying storage.
    /// In exchange, this means that the memory will not be reclaimed until
    /// all existing shards using it are dropped.
    fn split_inplace_at(self, at: usize) -> (Self::Shard, Self::Shard);
}

/// The raw guts of a Vec, used to free its allocation when all the shards are gone.
struct VecDropper<T> {
    ptr: *mut T,
    capacity: usize,
}

impl<T> Drop for VecDropper<T> {
    fn drop(&mut self) {
        unsafe {
            // Set len to 0 because we only want to free the memory.
            // Dropping the elements themselves is taken care of by the shards.
            mem::drop(Vec::from_raw_parts(self.ptr, 0, self.capacity));
        }
    }
}

/// A shard of a [`Vec<T>`](std::vec::Vec), can be used mostly like a Vec.
///
/// The major difference is that, when dropped, [`VecShard<T>`](crate::VecShard)
/// will not immediately free its allocated memory.
/// Instead, it will only drop all its items.
/// The memory itself will be freed once all VecShards from the Vec are gone.
pub struct VecShard<T> {
    dropper: Arc<VecDropper<T>>,

    data: *mut T,
    len: usize,
}

// These are the same as for Vec<T>
// Probably sound, since the only thing we share is the Arc
unsafe impl<T: Send> Send for VecShard<T> {}
unsafe impl<T: Sync> Sync for VecShard<T> {}

impl<T> VecShard<T> {
    fn into_raw_parts(self) -> (Arc<VecDropper<T>>, *mut T, usize) {
        let dropper = unsafe { ptr::read(&self.dropper as *const Arc<VecDropper<T>>) };
        let data = self.data;
        let len = self.len;
        mem::forget(self);
        (dropper, data, len)
    }

    /// Try to merge the given shards without moving them around.
    ///
    /// This can only succeed if `left` and `right` were split off from the same Vec
    /// and are directly adjacent to each other.
    /// Furthermore, `right` needs to be at a higher address than left so the elements stay in the right order.
    ///
    /// Returns the merged shard on success and an `Err` otherwise.
    ///
    /// This function will always run in O(1) time.
    pub fn merge_inplace(left: Self, right: Self) -> Result<Self, CantMerge<T, WouldMove>> {
        use WouldMove::*;
        // Are the shards even from the same Vec?
        if !Arc::ptr_eq(&left.dropper, &right.dropper) {
            Err(CantMerge {
                reason: DifferentAllocations,
                left,
                right,
            })
        } else if unsafe { left.data.add(left.len) } == right.data {
            let (ldropper, ldata, llen) = left.into_raw_parts();
            let (rdropper, _, rlen) = right.into_raw_parts();
            std::mem::drop(rdropper);
            Ok(VecShard {
                dropper: ldropper,
                data: ldata,
                len: llen + rlen,
            })
        } else if unsafe { right.data.add(right.len) } == left.data {
            Err(CantMerge {
                left,
                right,
                reason: WrongOrder,
            })
        } else {
            Err(CantMerge {
                reason: NotAdjacent,
                left,
                right,
            })
        }
    }

    /// Try to merge the given shards without allocating a new `Vec`.
    ///
    /// This function will always succeed if the passed shards can be merged in-place
    /// or if they're the only two shards within a Vec.
    ///
    /// Returns the merged shard on success and an `Err` otherwise.
    ///
    /// This function may take time line in the length of the input shards, but it will never allocate.
    pub fn merge_noalloc(left: Self, right: Self) -> Result<Self, CantMerge<T, WouldAlloc>> {
        use WouldMove::*;

        let cant_merge = match Self::merge_inplace(left, right) {
            // happy path
            Ok(shard) => return Ok(shard),
            Err(err) => err,
        };

        if cant_merge.reason == DifferentAllocations {
            return Err(CantMerge {
                left: cant_merge.left,
                right: cant_merge.right,
                reason: WouldAlloc::DifferentAllocations,
            });
        }

        let (ldropper, ldata, llen) = cant_merge.left.into_raw_parts();
        let (rdropper, rdata, rlen) = cant_merge.right.into_raw_parts();

        if cant_merge.reason == WrongOrder {
            // semi-fast path: we only need to rotate
            unsafe { slice::from_raw_parts_mut(rdata, llen + rlen).rotate_left(rlen) };
            Ok(VecShard {
                dropper: ldropper,
                data: rdata,
                len: llen + rlen,
            })
        } else if cant_merge.reason == NotAdjacent && Arc::strong_count(&ldropper) == 2 {
            // There are only 2 references to the dropper left,
            // and we're holding ldropper and rdropper, so we can freely re-use the allocation

            let new_data = unsafe {
                if rdata < ldata {
                    // If right is actually on the left side, we have to shuffle things around
                    if llen < rlen {
                        //  ...  |---------- r ----------| ... |------ l ------|
                        ptr::copy(ldata, rdata.add(rlen), llen);
                        //  ...  |---------- r ----------|------ l ------|  ...
                        slice::from_raw_parts_mut(rdata, rlen+llen).rotate_left(rlen);
                        //  ...  |------ l ------|---------- r ----------|  ...
                        rdata
                    } else {
                        //  ...  |------ r ------| ... |---------- l ----------|
                        ptr::copy(rdata, ldata.sub(rlen), rlen);
                        //  ...   ...  |------ r ------|---------- l ----------|
                        slice::from_raw_parts_mut(ldata.sub(rlen), rlen+llen).rotate_left(rlen);
                        //  ...   ...  |---------- l ----------|------ r ------|
                        ldata.sub(rlen)
                    }
                } else {
                    // Otherwise, just scootch it over
                    //  ...  |---------- l ----------|    ...  |------ r ------|
                    ptr::copy(rdata, ldata.add(llen), rlen);
                    //  ...  |---------- l ----------|------ r ------|   ...
                    ldata
                }
            };
            Ok(VecShard {
                data: new_data,
                len: llen + rlen,
                dropper: ldropper,
            })
        } else {
            Err(CantMerge {
                reason: WouldAlloc::OtherShardsLeft,
                left: VecShard {
                    dropper: ldropper,
                    data: ldata,
                    len: llen,
                },
                right: VecShard {
                    dropper: rdropper,
                    data: rdata,
                    len: rlen,
                },
            })
        }
    }

    /// Merge the given shards into a single shard.
    ///
    /// This will attempt an O(1) merge like `merge_inplace` but fall back to copying slices around
    /// within their allocation and possibly allocating a new Vec if needed.
    pub fn merge(left: Self, right: Self) -> Self {
        Self::merge_noalloc(left, right).unwrap_or_else(|err| {
            let (_ldropper, ldata, llen) = err.left.into_raw_parts();
            let (_rdropper, rdata, rlen) = err.right.into_raw_parts();

            // Give up and allocate
            let mut vec = Vec::with_capacity(llen + rlen);
            unsafe {
                ptr::copy(ldata, vec.as_mut_ptr(), llen);
                ptr::copy(rdata, vec.as_mut_ptr().add(llen), rlen);
                vec.set_len(llen + rlen);
            }
            Self::from(vec)
        })
    }
}

impl<T> ShardExt for VecShard<T> {
    type Shard = VecShard<T>;

    fn split_inplace_at(mut self, at: usize) -> (Self::Shard, Self::Shard) {
        assert!(at <= self.len);

        let right = VecShard {
            dropper: self.dropper.clone(),
            data: unsafe { self.data.add(at) },
            len: self.len - at,
        };

        // for the left shard, just cut ourselves down to size
        self.len = at;

        (self, right)
    }
}

impl<T> Drop for VecShard<T> {
    fn drop(&mut self) {
        // Drop all the elements
        // The VecDropper will take care of freeing the Vec itself, if needed
        for o in 0..self.len {
            unsafe { ptr::drop_in_place(self.data.add(o)) };
        }
    }
}

impl<T> Deref for VecShard<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.data, self.len) }
    }
}

impl<T> DerefMut for VecShard<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.data, self.len) }
    }
}

impl<T> AsRef<[T]> for VecShard<T> {
    fn as_ref(&self) -> &[T] {
        &*self
    }
}

impl<T> AsMut<[T]> for VecShard<T> {
    fn as_mut(&mut self) -> &mut [T] {
        &mut *self
    }
}

impl<T> Borrow<[T]> for VecShard<T> {
    fn borrow(&self) -> &[T] {
        &*self
    }
}

impl<T> BorrowMut<[T]> for VecShard<T> {
    fn borrow_mut(&mut self) -> &mut [T] {
        &mut *self
    }
}

impl<T, I: SliceIndex<[T]>> Index<I> for VecShard<T> {
    type Output = <I as slice::SliceIndex<[T]>>::Output;

    fn index(&self, idx: I) -> &Self::Output {
        &((**self)[idx])
    }
}

impl<T, I: SliceIndex<[T]>> IndexMut<I> for VecShard<T> {
    fn index_mut(&mut self, idx: I) -> &mut Self::Output {
        &mut ((**self)[idx])
    }
}

impl<T: PartialEq> PartialEq for VecShard<T> {
    fn eq(&self, rhs: &Self) -> bool {
        **self == **rhs
    }
}

impl<T: Eq> Eq for VecShard<T> {}

impl<T> Iterator for VecShard<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.len > 0 {
            let res = unsafe { self.data.read() };
            self.len -= 1;
            self.data = unsafe { self.data.add(1) };
            Some(res)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<T> ExactSizeIterator for VecShard<T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<T> DoubleEndedIterator for VecShard<T> {
    fn next_back(&mut self) -> Option<T> {
        if self.len > 0 {
            self.len -= 1;
            Some(unsafe { self.data.add(self.len).read() })
        } else {
            None
        }
    }
}

impl<T> FusedIterator for VecShard<T> {}

impl<T: Hash> Hash for VecShard<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&**self, state)
    }
}

impl<T> From<Vec<T>> for VecShard<T> {
    fn from(mut v: Vec<T>) -> Self {
        let res = VecShard {
            dropper: Arc::new(VecDropper {
                ptr: v.as_mut_ptr(),
                capacity: v.capacity(),
            }),
            data: v.as_mut_ptr(),
            len: v.len(),
        };
        mem::forget(v);
        res
    }
}

impl<T> Into<Vec<T>> for VecShard<T> {
    fn into(self) -> Vec<T> {
        // First, move everything out of self so we don't drop anything
        let (dropper, data, len) = self.into_raw_parts();

        // Optimization: if this shard is the only one left from the backing Vec, we re-use its allocation
        if let Ok(dropper) = Arc::try_unwrap(dropper) {
            // If our data is already at the start of the backing Vec, we don't need to move it
            if data != dropper.ptr {
                unsafe { ptr::copy(data, dropper.ptr, len) };
            }
            let v = unsafe { Vec::from_raw_parts(dropper.ptr, len, dropper.capacity) };
            // Make sure we don't drop anything that the new Vec will need
            mem::forget(dropper);
            v
        } else {
            // Otherwise, just allocate a new Vec
            let mut v = Vec::with_capacity(len);
            unsafe {
                ptr::copy_nonoverlapping(data, v.as_mut_ptr(), len);
                v.set_len(len);
            };
            v
        }
    }
}

impl<T: Clone> Clone for VecShard<T> {
    fn clone(&self) -> VecShard<T> {
        // Not much we can do here, just make a new Vec
        let mut vec = Vec::with_capacity(self.len);
        vec.extend_from_slice(unsafe { slice::from_raw_parts(self.data, self.len) });
        VecShard::from(vec)
    }
}

impl<T: fmt::Debug> fmt::Debug for VecShard<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", &**self)
    }
}

impl<T> ShardExt for Vec<T> {
    type Shard = VecShard<T>;

    fn split_inplace_at(self, at: usize) -> (Self::Shard, Self::Shard) {
        VecShard::from(self).split_inplace_at(at)
    }
}
