use crate::embedding::EmbeddingVector;
use crate::space::VectorSpace;

/// Result of KNN search
#[derive(Clone, Copy, Debug)]
pub struct KnnResult {
    pub index: u32,
    pub distance: f32,
}

/// Find k nearest neighbors to query embedding.
///
/// Uses cosine distance (1.0 - cosine_similarity) as metric.
/// Linear scan with insertion-sort maintained in the results buffer.
///
/// Returns actual number of results (min(k, count)).
pub fn knn(
    query: &EmbeddingVector,
    k: usize,
    space: &VectorSpace,
    results: &mut [KnnResult],
) -> usize {
    let count = space.count();
    if count == 0 || k == 0 || results.is_empty() {
        return 0;
    }

    let max_k = if k < results.len() { k } else { results.len() };
    let mut result_count: usize = 0;

    // Initialize results with maximum distance
    let mut i = 0u32;
    while i < count {
        let emb = match space.get_embedding(i) {
            Some(e) => e,
            None => { i += 1; continue; }
        };

        let distance = 1.0 - query.cosine_similarity(emb);

        // Insert into sorted results buffer (ascending by distance)
        if result_count < max_k {
            // Still have room, insert at correct position
            let mut pos = result_count;
            while pos > 0 && results[pos - 1].distance > distance {
                results[pos] = results[pos - 1];
                pos -= 1;
            }
            results[pos] = KnnResult { index: i, distance };
            result_count += 1;
        } else if distance < results[result_count - 1].distance {
            // Better than worst result, replace and re-sort
            let mut pos = result_count - 1;
            while pos > 0 && results[pos - 1].distance > distance {
                results[pos] = results[pos - 1];
                pos -= 1;
            }
            results[pos] = KnnResult { index: i, distance };
        }

        i += 1;
    }

    result_count
}
