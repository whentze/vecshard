use crate::VecShard;

use serde::{ser::{Serialize, Serializer, SerializeSeq}, de::{Deserialize, Deserializer}};

impl<T> Serialize for VecShard<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len))?;
        for e in &**self {
            seq.serialize_element(&e)?;
        }
        seq.end()
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for VecShard<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <Vec<T> as Deserialize>::deserialize::<D>(deserializer).map(|v| VecShard::from(v))
    }
}