use crate::{containers::ContainerError, identifiers::Identifier};

use super::*;

#[test]
fn provenance_error_display_inconsistency() {
    let id = Identifier::File("/test/path/file1.txt".to_string());
    let error = ProvenanceError::Inconsistency(ContainerError::NotRegistered(id.clone()));

    assert_eq!(
        format!("{}", error),
        format!(
            "Provenance inconsistency: {}",
            ContainerError::NotRegistered(id.clone())
        )
    );
}
