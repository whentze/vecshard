# vecshard

Split Vecs in O(1) time.

[!Crates.io](https://img.shields.io/crates/v/vecshard.svg)
[![Build Status](https://travis-ci.org/whentze/vecshard.svg?branch=master)](https://travis-ci.org/whentze/vecshard)
[![Coverage Status](https://coveralls.io/repos/github/whentze/vecshard/badge.svg?branch=master)](https://coveralls.io/github/whentze/vecshard?branch=master)
[![License: CC0-1.0](https://licensebuttons.net/l/zero/1.0/80x15.png)](http://creativecommons.org/publicdomain/zero/1.0/)

You can split a `Vec` into two using `Vec::split_off`,
but since most allocators can't just go and split up an allocation, this needs to allocate space
for a second `Vec` and, even worse, copy the relevant elements over, which takes O(n) time.
You could also split it into slices using `Vec::split_at` or
`Vec::split_at_mut`, but this will not give you owned
data you can move around or move out of at will.

This crate provides a way to split a `Vec` into two owned `VecShard`s that
behave similar to Vecs that takes constant time.
The catch is that the `VecShard`s use reference counting to determine when the last of them is dropped.
Only then is the memory from the original `Vec` deallocated.
The individual items in the shards, however, are dropped as soon as the shard is dropped.

This functionality is provided through an extension trait for `Vec`, `ShardExt`.

## Documentation

Please refer to the [API docs on docs.rs](https://docs.rs/vecshard).

## Basic Example

```rust
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

## Conversion

Shards can be freely converted both `From` and `Into` Vecs.
Note that the latter may need to allocate if there are other shards also using the shards allocation.

```rust

let vec = vec![1, 2, 3];
let shard = VecShard::from(vec);
let vec2 : Vec<_> = shard.into();
```

## Iteration

To iterate over a `VecShard`, you have several choices.
`VecShard<T>` itself is a draining `Iterator` and returns owned `T` instances,
removing them from its own storage.
If you only need `&T` or `&mut T`, you can deref it to a slice and iterate over that.
Finally, if you need an owning `Iterator` but do not want to drain the shard,
you can `clone` the shard and iterate over that.

```rust
let mut shard = VecShard::from(vec!['y', 'e', 'e', 't']);

assert_eq!(Some('y'), shard.next());
assert_eq!(Some('e'), shard.next());

assert_eq!(*shard, ['e', 't']);
```
