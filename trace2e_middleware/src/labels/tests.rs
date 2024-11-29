use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::{
    identifier::Identifier,
    labels::{provenance::Provenance, Compliance, Labels},
};

use super::compliance::ComplianceSettings;

#[test]
fn unit_labels_new() {
    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );

    let id1_labels = Labels::new(id1.clone());
    let id2_labels = Labels::new(id2.clone());
    let id3_labels = Labels::new(id3.clone());

    assert_eq!(id1_labels.get_prov().len(), 0);
    assert_eq!(id2_labels.get_prov().len(), 0);
    assert_eq!(id3_labels.get_prov().len(), 0);

    assert_eq!(id1_labels.get_identifier(), &id1);
    assert_eq!(id2_labels.get_identifier(), &id2);
    assert_eq!(id3_labels.get_identifier(), &id3);
}

#[test]
fn unit_labels_prov_update_basic() {
    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_file("/path/to/file2.txt".to_string());

    let mut id1_labels = Labels::new(id1.clone());
    let id2_labels = Labels::new(id2.clone());
    let mut id3_labels = Labels::new(id3.clone());

    id1_labels.update_prov(&id2_labels);
    id3_labels.update_prov(&id1_labels);

    assert_eq!(id1_labels.get_prov(), vec![id2_labels.compliance.clone()]);
    assert_eq!(id2_labels.get_prov(), vec![]);
    assert_eq!(
        id3_labels.get_prov(),
        vec![id1_labels.compliance.clone(), id2_labels.compliance.clone()]
    );
}

#[test]
fn unit_labels_prov_update_self_ref() {
    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());

    let mut id1_labels = Labels::new(id1.clone());
    let mut id2_labels = Labels::new(id2.clone());

    // unlikely, but useful to show update behavior in case of self-reference
    id1_labels.update_prov(&id1_labels.clone());
    id2_labels.update_prov(&id2_labels.clone());
    assert_eq!(id1_labels.get_prov().len(), 0);
    assert_eq!(id2_labels.get_prov().len(), 0);

    id1_labels.update_prov(&id2_labels);
    id2_labels.update_prov(&id1_labels);
    assert_eq!(id1_labels.get_prov(), vec![id2_labels.compliance.clone()]);
    assert_eq!(id2_labels.get_prov(), vec![id1_labels.compliance]);
}

#[test]
fn unit_labels_prov_clear() {
    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );

    let mut id1_labels = Labels::new(id1.clone());
    let mut id2_labels = Labels::new(id2.clone());
    let mut id3_labels = Labels::new(id3.clone());

    id1_labels.update_prov(&id2_labels);
    id2_labels.update_prov(&id1_labels);
    id3_labels.update_prov(&id1_labels);

    assert_eq!(id1_labels.get_prov(), vec![id2_labels.compliance.clone()]);
    assert_eq!(id2_labels.get_prov(), vec![id1_labels.compliance.clone()]);
    assert_eq!(
        id3_labels.get_prov(),
        vec![id1_labels.compliance.clone(), id2_labels.compliance.clone()]
    );

    // Clear prov
    id1_labels.clear_prov();
    id2_labels.clear_prov();
    id3_labels.clear_prov();

    assert_eq!(id1_labels.get_prov(), vec![id2_labels.compliance.clone()]);
    assert_eq!(id2_labels.get_prov(), vec![id1_labels.compliance.clone()]);
    assert_eq!(id3_labels.get_prov(), vec![]);
}

#[test]
fn unit_labels_compliance_local_confidentiality_default() {
    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );

    let mut id1_labels = Labels::new(id1.clone());
    let id2_labels = Labels::new(id2.clone());
    let mut id3_labels = Labels::new(id3.clone());

    assert!(id1_labels.is_compliant(&id2_labels));
    id1_labels.update_prov(&id2_labels);
    assert!(id3_labels.is_compliant(&id1_labels));
    id3_labels.update_prov(&id1_labels);
}

#[test]
fn unit_labels_compliance_local_confidentiality_direct() {
    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );

    let mut id1_labels = Labels::new(id1.clone());
    let id2_labels = Labels::new(id2.clone());
    let id3_labels = Labels::new(id3.clone());

    // Set local confidentiality for process to prevent any leakage outside localhost
    id1_labels.set_local_confidentiality(true);

    assert!(id1_labels.is_compliant(&id2_labels));
    id1_labels.update_prov(&id2_labels);
    assert!(!id3_labels.is_compliant(&id1_labels));
}

#[test]
fn unit_labels_compliance_local_confidentiality_prov() {
    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );

    let mut id1_labels = Labels::new(id1.clone());
    let mut id2_labels = Labels::new(id2.clone());
    let id3_labels = Labels::new(id3.clone());

    // Set local confidentiality for file to prevent any leakage outside localhost
    id2_labels.set_local_confidentiality(true);

    assert!(id1_labels.is_compliant(&id2_labels));
    id1_labels.update_prov(&id2_labels);
    assert!(!id3_labels.is_compliant(&id1_labels));
}

#[test]
fn unit_labels_compliance_local_integrity_default() {
    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let mut id1_labels = Labels::new(id1.clone());
    let mut id2_labels = Labels::new(id2.clone());
    let mut id3_labels = Labels::new(id3.clone());

    let mut id4 = Identifier::new_process(1, 1, String::new());
    id4.mock_node(String::from("foreign"));
    let mut id5 = Identifier::new_file("/path/to/file2.txt".to_string());
    id5.mock_node(String::from("foreign"));

    let mut id4_labels = Labels::new(id4.clone());
    let id5_labels = Labels::new(id5.clone());

    assert!(id4_labels.is_compliant(&id5_labels));
    id4_labels.update_prov(&id5_labels);
    // Shortcut for the purpose of the unit test
    assert!(id3_labels.is_compliant(&id4_labels));
    id3_labels.update_prov(&id4_labels);

    // No local_integrity -> flows from external ressources allowed
    assert!(id1_labels.is_compliant(&id3_labels));
    id1_labels.update_prov(&id3_labels);
    assert!(id2_labels.is_compliant(&id1_labels));
    id2_labels.update_prov(&id1_labels);
}

#[test]
fn unit_labels_compliance_local_integrity_direct() {
    let id3 = Identifier::new_stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let mut id3_labels = Labels::new(id3.clone());

    let mut id4 = Identifier::new_process(1, 1, String::new());
    id4.mock_node(String::from("foreign"));
    let mut id5 = Identifier::new_file("/path/to/file2.txt".to_string());
    id5.mock_node(String::from("foreign"));

    let mut id4_labels = Labels::new(id4.clone());
    let id5_labels = Labels::new(id5.clone());

    // local_integrity on stream to demonstrate integrity enforcement based on source compliance label
    id3_labels.set_local_integrity(true);


    assert!(id4_labels.is_compliant(&id5_labels));
    id4_labels.update_prov(&id5_labels);

    // Shortcut for the purpose of the unit test
    // local_integrity -> not compliant flow
    assert!(!id3_labels.is_compliant(&id4_labels));
}

#[test]
fn unit_labels_compliance_local_integrity_prov() {
    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let mut id1_labels = Labels::new(id1.clone());
    let mut id2_labels = Labels::new(id2.clone());
    let mut id3_labels = Labels::new(id3.clone());

    let mut id4 = Identifier::new_process(1, 1, String::new());
    id4.mock_node(String::from("foreign"));
    let mut id5 = Identifier::new_file("/path/to/file2.txt".to_string());
    id5.mock_node(String::from("foreign"));

    let mut id4_labels = Labels::new(id4.clone());
    let id5_labels = Labels::new(id5.clone());

    // local_integrity on file to demonstrate integrity enforcement based on source provenance label
    id2_labels.set_local_integrity(true);

    assert!(id4_labels.is_compliant(&id5_labels));
    id4_labels.update_prov(&id5_labels);
    // Shortcut for the purpose of the unit test
    assert!(id3_labels.is_compliant(&id4_labels));
    id3_labels.update_prov(&id4_labels);

    assert!(id1_labels.is_compliant(&id3_labels));
    id1_labels.update_prov(&id3_labels);

    // local_integrity -> not compliant flow
    assert!(!id2_labels.is_compliant(&id1_labels));
}