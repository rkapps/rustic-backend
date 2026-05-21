/// Score every candidate in `candidates` against the query vector `vec_a`
/// using cosine similarity, then return the `top_k` highest-scoring items in
/// descending order.
///
/// `K` is the identifier type (e.g. `String`, `Uuid`) stored alongside each
/// candidate vector.  The returned pairs are `(id, score)`.
///
/// If `top_k` is larger than the number of candidates, all candidates are
/// returned (no padding).
pub fn search<K: Clone>(
    vec_a: &[f32],
    candidates: &[(K, Vec<f32>)],
    top_k: usize,
) -> Vec<(K, f32)> {
    let mut scores: Vec<(K, f32)> = candidates
        .iter()
        .map(|(id, vec)| (id.clone(), cosine_similarity(vec_a, vec)))
        .collect();

    scores.sort_by(|a, b: &(K, f32)| b.1.partial_cmp(&a.1).unwrap());
    scores.truncate(top_k);
    scores
}

/// Cosine similarity between two vectors: `dot(a, b) / (|a| * |b|)`.
///
/// Returns a value in `[-1.0, 1.0]` where `1.0` means identical direction.
/// Returns `0.0` when either vector is the zero vector to avoid division by
/// zero.
pub fn cosine_similarity(vec_a: &[f32], vec_b: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for (val_a, val_b) in vec_a.iter().zip(vec_b.iter()) {
        dot += val_a * val_b;
        norm_a += val_a * val_a;
        norm_b += val_b * val_b;
    }

    let magnitude = (norm_a * norm_b).sqrt();
    if magnitude == 0.0 {
        0.0
    } else {
        dot / magnitude
    }
}

#[cfg(test)]
mod tests {
    use crate::search::similarity::{cosine_similarity, search};

    #[test]
    fn test_vector_search_top_k() {
        let vec_a = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            (1, vec![1.0, 0.0, 2.0]),
            (2, vec![1.0, 2.0, 3.0]),
            (3, vec![1.0, 3.0, 4.0]),
            (4, vec![1.0, 3.0, 5.0]),
        ];

        let results = search(&vec_a, &candidates, 2);
        assert!(results.len() == 2);
        assert_eq!(results[0].0, 1);
        assert_eq!(results[1].0, 2);
        println!("{:?}", results);
    }

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let vec_a: Vec<f32> = vec![1.0, 2.0, 3.0];
        let vec_b: Vec<f32> = vec![1.0, 2.0, 3.0];
        let similarity = cosine_similarity(&vec_a, &vec_b);
        assert!((similarity - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let vec_a: Vec<f32> = vec![1.0, 2.0, 3.0];
        let vec_b: Vec<f32> = vec![-1.0, -2.0, -3.0];
        let similarity = cosine_similarity(&vec_a, &vec_b);
        assert!((similarity - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let vec_a = vec![1.0, 2.0, 3.0];
        let vec_b = vec![0.0, 0.0, 0.0];

        let similarity = cosine_similarity(&vec_a, &vec_b);
        assert_eq!(similarity, 0.0); // Should return 0.0
    }
}
