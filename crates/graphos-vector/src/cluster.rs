use crate::embedding::EmbeddingVector;
use crate::space::VectorSpace;

const MAX_CLUSTERS: usize = 16;
const MAX_NODES_PER_CLUSTER: usize = 32;

/// Maximum nodes supported for DBSCAN labeling
const MAX_DBSCAN_NODES: usize = 1024;

/// A cluster of nodes
#[derive(Clone, Copy)]
pub struct Cluster {
    pub label: u32,
    pub member_count: u32,
    pub members: [u32; MAX_NODES_PER_CLUSTER],
    pub centroid: EmbeddingVector,
}

impl Cluster {
    const fn empty() -> Self {
        Self {
            label: 0,
            member_count: 0,
            members: [0; MAX_NODES_PER_CLUSTER],
            centroid: EmbeddingVector::zero(),
        }
    }
}

/// DBSCAN result
pub struct ClusterResult {
    pub clusters: [Cluster; MAX_CLUSTERS],
    pub cluster_count: u32,
    pub noise_count: u32,
}

/// Node state for DBSCAN
const UNVISITED: u8 = 0;
const NOISE: u8 = 1;
const CLUSTERED: u8 = 2;

/// Run DBSCAN clustering.
///
/// eps: maximum cosine distance for neighborhood
/// min_points: minimum neighbors to form a cluster
pub fn dbscan(
    space: &VectorSpace,
    eps: f32,
    min_points: u32,
) -> ClusterResult {
    let count = space.count() as usize;
    let n = if count > MAX_DBSCAN_NODES { MAX_DBSCAN_NODES } else { count };

    let mut result = ClusterResult {
        clusters: [Cluster::empty(); MAX_CLUSTERS],
        cluster_count: 0,
        noise_count: 0,
    };

    if n == 0 {
        return result;
    }

    // Labels for each node
    let mut labels = [UNVISITED; MAX_DBSCAN_NODES];
    let mut current_cluster: u32 = 0;

    let mut i = 0;
    while i < n {
        if labels[i] != UNVISITED {
            i += 1;
            continue;
        }

        // Count neighbors within eps
        let emb_i = match space.get_embedding(i as u32) {
            Some(e) => e,
            None => { i += 1; continue; }
        };

        // Find all neighbors
        let mut neighbor_count: u32 = 0;
        let mut neighbors = [0u32; MAX_DBSCAN_NODES];
        let mut j = 0;
        while j < n {
            if j != i {
                if let Some(emb_j) = space.get_embedding(j as u32) {
                    let dist = 1.0 - emb_i.cosine_similarity(emb_j);
                    if dist <= eps {
                        if (neighbor_count as usize) < MAX_DBSCAN_NODES {
                            neighbors[neighbor_count as usize] = j as u32;
                            neighbor_count += 1;
                        }
                    }
                }
            }
            j += 1;
        }

        if neighbor_count < min_points {
            labels[i] = NOISE;
            i += 1;
            continue;
        }

        // Start a new cluster
        if current_cluster as usize >= MAX_CLUSTERS {
            // No more cluster slots available
            labels[i] = NOISE;
            i += 1;
            continue;
        }

        let cluster_idx = current_cluster as usize;
        result.clusters[cluster_idx].label = current_cluster;
        labels[i] = CLUSTERED;

        // Add core point to cluster
        if (result.clusters[cluster_idx].member_count as usize) < MAX_NODES_PER_CLUSTER {
            let mc = result.clusters[cluster_idx].member_count as usize;
            result.clusters[cluster_idx].members[mc] = i as u32;
            result.clusters[cluster_idx].member_count += 1;
        }

        // Expand cluster: add neighbors and check their neighborhoods (1 level)
        let mut ni = 0u32;
        while ni < neighbor_count {
            let nb = neighbors[ni as usize] as usize;
            if nb >= n {
                ni += 1;
                continue;
            }

            if labels[nb] == NOISE {
                // Border point: add to cluster
                labels[nb] = CLUSTERED;
                if (result.clusters[cluster_idx].member_count as usize) < MAX_NODES_PER_CLUSTER {
                    let mc = result.clusters[cluster_idx].member_count as usize;
                    result.clusters[cluster_idx].members[mc] = nb as u32;
                    result.clusters[cluster_idx].member_count += 1;
                }
            } else if labels[nb] == UNVISITED {
                labels[nb] = CLUSTERED;
                if (result.clusters[cluster_idx].member_count as usize) < MAX_NODES_PER_CLUSTER {
                    let mc = result.clusters[cluster_idx].member_count as usize;
                    result.clusters[cluster_idx].members[mc] = nb as u32;
                    result.clusters[cluster_idx].member_count += 1;
                }

                // Check if nb is also a core point (expand)
                if let Some(emb_nb) = space.get_embedding(nb as u32) {
                    let mut nb_neighbor_count: u32 = 0;
                    let mut k = 0;
                    while k < n {
                        if k != nb {
                            if let Some(emb_k) = space.get_embedding(k as u32) {
                                let dist = 1.0 - emb_nb.cosine_similarity(emb_k);
                                if dist <= eps {
                                    nb_neighbor_count += 1;
                                    // Add new neighbor to processing queue if not already clustered
                                    if nb_neighbor_count >= min_points
                                        && labels[k] == UNVISITED
                                        && (neighbor_count as usize) < MAX_DBSCAN_NODES
                                    {
                                        neighbors[neighbor_count as usize] = k as u32;
                                        neighbor_count += 1;
                                    }
                                }
                            }
                        }
                        k += 1;
                    }
                }
            }
            ni += 1;
        }

        current_cluster += 1;
        i += 1;
    }

    result.cluster_count = current_cluster;

    // Count noise
    let mut noise: u32 = 0;
    let mut idx = 0;
    while idx < n {
        if labels[idx] == NOISE {
            noise += 1;
        }
        idx += 1;
    }
    result.noise_count = noise;

    // Compute centroids
    let mut c = 0;
    while c < current_cluster as usize {
        let cluster = &result.clusters[c];
        let mc = cluster.member_count as usize;
        if mc > 0 {
            let mut centroid = EmbeddingVector::zero();
            let mut m = 0;
            while m < mc {
                let slot = cluster.members[m];
                if let Some(emb) = space.get_embedding(slot) {
                    centroid.add_scaled(emb, 1.0);
                }
                m += 1;
            }
            centroid.scale(1.0 / mc as f32);
            centroid.normalize();
            result.clusters[c].centroid = centroid;
        }
        c += 1;
    }

    result
}
