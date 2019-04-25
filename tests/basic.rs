use vecshard::{ShardExt, VecShard};

#[test]
fn deref() {
    let vec = vec![1, 2, 3, 4, 5, 6];
    let (left, mut right) = vec.split_inplace_at(3);

    assert_eq!(&*left, &[1, 2, 3]);
    assert_eq!(right[0], 4);

    right[0] = 5;
    right[1] = 8;
    right[2] = 13;

    let fib = VecShard::merge(left, right);
    assert_eq!(*fib, [1, 2, 3, 5, 8, 13]);
}

#[test]
fn vec_roundtrip() {
    let vec = vec!["ja", "da", "meint", "der", "ich", "hät'", "abgeschmatzt"];

    let shard = VecShard::from(vec.clone());
    let vec2: Vec<_> = shard.into();
    assert_eq!(vec, vec2);
}

#[test]
fn into_vecs() {
    let (left, right) = vec![1, 11, 21, 12_11, 11_12_21, 31_22_11].split_inplace_at(3);

    // this one needs to allocate a new Vec, since right still exists
    let lvec: Vec<_> = left.into();
    // this one is now the only shard left and can re-use the allocation
    let rvec: Vec<_> = right.into();

    assert_eq!(lvec, [1, 11, 21]);
    assert_eq!(rvec, [12_11, 11_12_21, 31_22_11]);
}

#[test]
fn things_get_dropped() {
    use std::rc::Rc;

    // Idea: make one Rc, then clone it a bunch of times into the Vec
    let rc = Rc::new(());

    let rcs = vec![rc.clone(); 20];

    let (left, right) = rcs.split_inplace_at(10);

    // Drop the left half
    std::mem::drop(left);

    // Drain the right half
    for x in right {
        assert_eq!(*x, ());
    }

    // If everything in the Vec got dropped, then the refcount
    // should be 1 again, and we can unwrap it.
    Rc::try_unwrap(rc).unwrap();
}

#[test]
fn clone_works() {
    let vec = vec![1, 2, 6, 24, 120];
    let (left, _) = vec.split_inplace_at(3);

    assert_eq!(left, left.clone());
    assert_eq!(*left, [1, 2, 6]);
}

#[test]
fn debug_looks_ok() {
    use std::fmt::Write;
    let shard = VecShard::from(vec![1, 3, 1, 2]);

    let mut buf = String::with_capacity(16);
    write!(buf, "{:?}", shard).unwrap();

    assert_eq!(buf, "[1, 3, 1, 2]");
}

#[test]
fn lucky_merges() {
    let dish = vec!["mashed potatoes", "liquor", "pie", "jellied eels"];
    let clone = dish.clone();
    let old_ptr = clone.as_ptr();

    let (rest, right) = clone.split_inplace_at(2);
    let (left, middle) = rest.split_inplace_at(1);

    let eww = VecShard::merge_inplace(middle, right).unwrap();
    let new_dish: Vec<_> = VecShard::merge_inplace(left, eww).unwrap().into();
    let new_ptr = new_dish.as_ptr();

    assert_eq!(dish, new_dish);

    // assert that the new vec lives at the same pointer
    // if this succeeds, it's likely that we didn't have to allocate for it
    assert_eq!(old_ptr, new_ptr);
}

#[test]
fn unlucky_merges() {
    use vecshard::error::{CantMerge, WouldMove::*};

    let vec = vec![1, 3, 6, 10, 15, 21, 28, 36];

    let (left, rest) = vec.clone().split_inplace_at(4);
    let (middle, right) = rest.split_inplace_at(2);

    let CantMerge {
        left: right,
        right: middle,
        reason,
    } = VecShard::merge_inplace(right, middle).unwrap_err();

    assert_eq!(reason, WrongOrder);

    let CantMerge {
        left,
        right,
        reason,
    } = VecShard::merge_noalloc(left, right).unwrap_err();

    assert_eq!(reason.to_string(), "the two shards are not directly adjacent in memory and can't be moved around because there are still other shards in the Vec");

    let CantMerge {
        right: left,
        reason,
        ..
    } = VecShard::merge_inplace(middle, left).unwrap_err();

    assert_eq!(
        reason.to_string(),
        "the two shards are adjacent, but were passed in the reverse order."
    );

    let CantMerge { left, reason, .. } = VecShard::merge_inplace(left, right).unwrap_err();

    assert_eq!(
        reason.to_string(),
        "the two shards are not directly adjacent in memory."
    );

    let different = VecShard::from(vec![4, 5, 2, 5, 7]);

    let err = VecShard::merge_inplace(left, different).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Can't perform quick merge because the two shards are not from the same memory allocation."
    );
    // no-alloc should not work in this case either
    let err = VecShard::merge_noalloc(err.left, err.right).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Can't perform quick merge because the two shards are not from the same memory allocation."
    );
}

#[test]
fn weird_merges() {
    let vec = vec![1, 4, 9, 16, 25, 36, 49, 64];

    let (left, right) = vec.clone().split_inplace_at(4);

    // merge in reverse order
    let big = VecShard::merge(right, left);

    assert_eq!(*big, [25, 36, 49, 64, 1, 4, 9, 16]);

    // split in three shards
    let (left, rest) = vec.clone().split_inplace_at(4);
    let (middle, right) = rest.split_inplace_at(2);

    // then merge the outer ones together first
    let outer = VecShard::merge(left, right);
    let big = VecShard::merge(outer, middle);

    assert_eq!(*big, [1, 4, 9, 16, 49, 64, 25, 36]);

    // split in three shards, drop the middle to free the space
    let (left, rest) = vec.clone().split_inplace_at(4);
    let (middle, right) = rest.split_inplace_at(2);
    std::mem::drop(middle);

    // then merge the outer ones together
    let outer = VecShard::merge(left, right);

    assert_eq!(*outer, [1, 4, 9, 16, 49, 64]);

    // same as before
    let (left, rest) = vec.clone().split_inplace_at(4);
    let (middle, right) = rest.split_inplace_at(2);
    std::mem::drop(middle);

    // but merge in reverse order
    let outer = VecShard::merge(right, left);

    assert_eq!(*outer, [49, 64, 1, 4, 9, 16]);

    // same as before, but split right off first
    let (rest, right) = vec.clone().split_inplace_at(4);
    let (left, middle) = rest.split_inplace_at(2);
    std::mem::drop(middle);

    let outer = VecShard::merge(right, left);

    assert_eq!(*outer, [25, 36, 49, 64, 1, 4]);
}

#[test]
fn backwards_iteration() {
    let mut shard = VecShard::from(vec![0, 1, 2, 3, 4]);

    assert_eq!(shard.next(), Some(0));
    assert_eq!(shard.next_back(), Some(4));
    assert_eq!(shard.next_back(), Some(3));
    assert_eq!(shard.next(), Some(1));
    assert_eq!(shard.next_back(), Some(2));
    assert_eq!(shard.next_back(), None);
}

#[test]
fn lens_match_up() {
    let s1 = VecShard::from(vec![]);
    let s2 = VecShard::from(vec![0]);
    let (s3, s4) = vec![0; 100].split_inplace_at(10);
    let mut s5 = s3.clone();
    s5.next();
    s5.next_back();

    for shard in &[s1, s2, s3, s4, s5] {
        assert_eq!(shard.len(), ExactSizeIterator::len(shard));
        assert_eq!(shard.len(), shard.size_hint().0);
        assert_eq!(shard.len(), shard.size_hint().1.unwrap());
    }
}
