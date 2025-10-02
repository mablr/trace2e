//! Core services for the traceability middleware.
//!
//! This module contains the internal service implementations that provide
//! the core functionality of the traceability system:
//!
//! - **Sequencer**: Flow coordination and resource reservation management
//! - **Provenance**: Data lineage tracking across resources and nodes
//! - **Compliance**: Policy management and enforcement
//! - **Consent**: User consent management for data flows

pub mod compliance;
pub mod consent;
pub mod provenance;
pub mod sequencer;
