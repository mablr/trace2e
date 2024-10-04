use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::{
    identifier::Identifier,
    labels::{provenance::Provenance, Compliance, ConfidentialityLabel, Labels},
};

#[test]
fn unit_labels_new() {
    let id1 = Identifier::new_process(1, 1);
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );

    let id1_labels = Labels::new(id1.clone());
    let id2_labels = Labels::new(id2.clone());
    let id3_labels = Labels::new(id3.clone());

    assert_eq!(id1_labels.get_prov(), vec![]);
    assert_eq!(id2_labels.get_prov(), vec![id2.clone()]);
    assert_eq!(id3_labels.get_prov(), vec![]);
}

#[test]
fn unit_labels_set() {
    let id1 = Identifier::new_process(1, 1);
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );

    let mut labels = Labels::new(id1.clone());
    assert_eq!(labels.get_prov(), vec![]);

    labels.set_prov(vec![id2.clone(), id3.clone()]);
    assert_eq!(labels.get_prov(), vec![id2.clone(), id3.clone()]);
}

#[test]
fn unit_labels_update() {
    let id1 = Identifier::new_process(1, 1);
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_file("/path/to/file2.txt".to_string());

    let mut id1_labels = Labels::new(id1.clone());
    let id2_labels = Labels::new(id2.clone());
    let mut id3_labels = Labels::new(id3.clone());

    id1_labels.update_prov(&id2_labels);
    id3_labels.update_prov(&id1_labels);

    assert_eq!(id1_labels.get_prov(), vec![id2.clone()]);
    assert_eq!(id2_labels.get_prov(), vec![id2.clone()]);
    assert_eq!(id3_labels.get_prov(), vec![id3.clone(), id2.clone()]);
}

#[test]
fn unit_labels_confidentiality() {
    let id1 = Identifier::new_process(1, 1);
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());

    let id1_labels = Labels::new(id1.clone());
    let mut id2_labels = Labels::new(id2.clone());

    assert_eq!(id2_labels.is_flow_allowed(id1_labels.clone()), true);

    id2_labels.confidentiality = ConfidentialityLabel::High;

    assert_eq!(id2_labels.is_flow_allowed(id1_labels), false);
}