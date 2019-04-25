#![cfg(feature = "serde")]

use serde_test::{assert_tokens, Token};
use vecshard::VecShard;

#[test]
fn serde_empty() {
    let shard = VecShard::from(Vec::<u64>::new());

    assert_tokens(&shard, &[Token::Seq { len: Some(0) }, Token::SeqEnd]);
}

#[test]
fn serde_chars() {
    let shard = VecShard::from(vec!['a', 'b', 'c']);

    assert_tokens(
        &shard,
        &[
            Token::Seq { len: Some(3) },
            Token::Char('a'),
            Token::Char('b'),
            Token::Char('c'),
            Token::SeqEnd,
        ],
    );
}
