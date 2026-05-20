use crate::core::{repository::Searchable, search::SortField};


/// Sort `items` in-place according to an ordered list of [`SortField`]s.
///
/// Fields are compared left-to-right; ties in earlier fields are broken by
/// subsequent ones.  Items whose `get_field_value` returns `None` for the
/// current sort field are treated as equal and fall through to the next field.
pub fn apply_sort<M: Searchable>(mut items: Vec<M>, sort_fields: &[SortField]) -> Vec<M> {
    items.sort_by(|a, b| {
        for sort_field in sort_fields {
            let val_a = a.get_field_value(&sort_field.field);
            let val_b = b.get_field_value(&sort_field.field);

            if let (Some(a), Some(b)) = (val_a, val_b) {
                let ordering = if sort_field.ascending {
                    a.cmp(&b)
                } else {
                    b.cmp(&a)
                };

                if ordering != std::cmp::Ordering::Equal {
                    return ordering;
                }
            }
        }
        std::cmp::Ordering::Equal
    });

    items
}
