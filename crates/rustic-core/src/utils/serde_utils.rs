use serde::Serialize;

pub fn serialize_vec_or_null<T, S>(vec: &Vec<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: serde::Serializer,
{
    if vec.is_empty() {
        serializer.serialize_none()
    } else {
        vec.serialize(serializer)
    }
}
