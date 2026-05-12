#![no_std]

pub mod error;
pub mod embedding;
pub mod space;
pub mod physics;
pub mod knn;
pub mod cluster;

pub use error::VectorError;
pub use embedding::EmbeddingVector;
pub use space::{VectorSpace, Position3D, Velocity3D, PhysicsConfig};
pub use physics::physics_step;
pub use knn::{knn, KnnResult};
pub use cluster::{dbscan, Cluster, ClusterResult};
