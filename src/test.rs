use crate::{ShardExt, VecShard};

#[test]
fn simple_deref() {
    let vec = vec![0, 1, 2, 3, 4, 5];
    let (left, right) = vec.split_inplace_at(3);
    assert_eq!(&*left, &[0, 1, 2]);
    assert_eq!(&*right, &[3, 4, 5]);
}

#[test]
fn vec_roundtrip() {
    let vec = vec!["ja", "da", "meint", "der", "ich", "hät'", "abgeschmatzt"];

    let shard = VecShard::from(vec.clone());
    let vec2: Vec<_> = shard.into();
    assert_eq!(vec, vec2);
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
fn debug_looks_ok() {
    use std::fmt::Write;
    let shard = VecShard::from(vec![1, 3, 1, 2]);

    let mut buf = String::with_capacity(16);
    write!(buf, "{:?}", shard).unwrap();

    assert_eq!(buf, "[1, 3, 1, 2]");
}

#[test]
fn lucky_merges() {
    use crate::merge_shards;

    let dish = vec!["mashed potatoes", "liquor", "pie", "jellied eels"];
    let old_ptr = dish.as_ptr();

    let (rest, right) = dish.split_inplace_at(2);
    let (left, middle) = rest.split_inplace_at(1);

    let eww = merge_shards(middle, right);
    let dish: Vec<_> = merge_shards(left, eww).into();
    let new_ptr = dish.as_ptr();

    // assert that the new vec lives at the same pointer
    // if this succeeds, it's likely that we didn't have to allocate for it
    assert_eq!(old_ptr, new_ptr);
}
