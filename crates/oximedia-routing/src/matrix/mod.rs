//! Matrix routing module for crosspoint routing and connection management.

pub mod connection;
pub mod crosspoint;
pub mod solver;

pub use connection::{
    Connection, ConnectionId, ConnectionManager, ConnectionType, RoutingPriority,
};
pub use crosspoint::{CrosspointId, CrosspointMatrix, CrosspointState, MatrixError};
pub use solver::{RoutingPath, RoutingPathSolver};
