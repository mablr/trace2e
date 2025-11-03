use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};

use crate::traceability::services::compliance::{DeletionPolicy, Policy};
use crate::transport::loopback::spawn_loopback_middlewares;

use super::fixtures::{FileMapping, StreamMapping};

#[tokio::test]
async fn integration_broadcast_deletion_three_nodes() {
    // Broadcast deletion integration test with three nodes
    //
    // This test verifies that:
    // 1. A file on node1 can be marked for deletion via BroadcastDeletion API
    // 2. The deletion status is marked on all nodes (broadcasted successfully)
    // 3. All IO operations that involve the deleted resource are refused
    //
    // Test topology:
    // Node1: File flows out through socket1337 to Node2
    // Node2: Receives data from Node1, forwards through socket1339 to Node3
    // Node3: Receives data from Node2
    //
    // After deletion:
    // - Node1: File is marked as Pending deletion
    // - Node2: Receives BroadcastDeletion notification
    // - Node3: Receives BroadcastDeletion notification
    // - Further operations on the data chain are refused
    //
    // flowchart LR
    //     s1337on1["socket1337 on Node1"] --- s1338on2["socket1338 on Node2"]
    //     s1339on2["socket1339 on Node2"] --- s1340on3["socket1340 on Node3"]
    //     F1_1_1["File1 (pid=1, fd=4) @Node1"] -- 1: read --> P1on1["Process1 @Node1"]
    //     P1on1 -- 2: write --> s1337on1
    //     s1338on2 -- 3: read --> P2on2["Process2 @Node2"]
    //     P2on2 -- 4: write --> s1339on2
    //     s1340on3 -- 5: read --> P3on3["Process3 @Node3"]
    //     policy0(["6: BroadcastDeletion(File1)"]) -. .- F1_1_1
    //     F1_1_1 -- 7: blocked --x P1on1
    //     P2on2 -- 8: blocked --x s1339on2
    //     P3on3 -- 9: blocked --x operations

    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();

    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string(), "10.0.0.3".to_string()];
    let mut middlewares =
        spawn_loopback_middlewares(ips.clone()).await.into_iter().map(|(p2m, o2m)| {
            (
                ServiceBuilder::new()
                    .layer(TimeoutLayer::new(Duration::from_millis(100)))
                    .service(p2m),
                o2m,
            )
        });

    let (mut p2m_1, mut o2m_1) = middlewares.next().unwrap();
    let (mut p2m_2, _) = middlewares.next().unwrap();
    let (mut p2m_3, _) = middlewares.next().unwrap();

    // Setup: Create file on node1
    let fd1_1_1 = FileMapping::new(1, 4, "/tmp/broadcast_delete.txt", "10.0.0.1".to_string());
    local_enroll!(p2m_1, fd1_1_1);

    // Setup: Create streams connecting the three nodes in a chain
    let stream1_2 = StreamMapping::new(1, 3, "10.0.0.1:1337", "10.0.0.2:1338");
    let stream2_1 = StreamMapping::new(2, 3, "10.0.0.2:1338", "10.0.0.1:1337");
    let stream2_3 = StreamMapping::new(2, 4, "10.0.0.2:1339", "10.0.0.3:1340");
    let stream3_2 = StreamMapping::new(3, 3, "10.0.0.3:1340", "10.0.0.2:1339");

    remote_enroll!(p2m_1, stream1_2);
    remote_enroll!(p2m_2, stream2_1);
    remote_enroll!(p2m_2, stream2_3);
    remote_enroll!(p2m_3, stream3_2);

    // Step 1-5: Establish initial data flow from file through the three-node chain
    // This creates provenance chains on each node
    read!(p2m_1, fd1_1_1);
    write!(p2m_1, stream1_2);
    read!(p2m_2, stream2_1);
    write!(p2m_2, stream2_3);
    read!(p2m_3, stream3_2);

    // Verify: File has initial policy (NotDeleted)
    assert_policies!(
        o2m_1,
        HashSet::from([fd1_1_1.file()]),
        HashMap::from([(fd1_1_1.localized_file(), Policy::default())])
    );

    // Step 6: Broadcast deletion of the file to all nodes
    // This should mark the file as Pending deletion on Node1
    // and notify all other nodes
    broadcast_deletion!(o2m_1, fd1_1_1.file());

    // Verify: The file is marked as Pending deletion on node1 (where it originated)
    assert_policies!(
        o2m_1,
        HashSet::from([fd1_1_1.file()]),
        HashMap::from([(
            fd1_1_1.localized_file(),
            Policy::new(Default::default(), Default::default(), DeletionPolicy::Pending, false)
        )])
    );

    // Step 7-9: Verify that operations involving the deleted resource are refused
    // After broadcast deletion, any operations on the data chain should be refused

    // Attempt: Read from stream on node2 that receives data from deleted file
    // This should be refused because the source (file on node1) is deleted
    assert_eq!(
        read_request!(p2m_2, stream2_1),
        u128::MAX,
        "Node2 should refuse to read from stream with deleted source"
    );

    // Attempt: Write from process on node2 to next stream
    // This should be refused because the data chain includes a deleted source
    assert_eq!(
        write_request!(p2m_2, stream2_3),
        u128::MAX,
        "Node2 should refuse to write to next stream when data chain includes deleted source"
    );

    // Attempt: Read from stream on node3 that receives from node2
    // This should be refused because the original source (file on node1) is deleted
    assert_eq!(
        read_request!(p2m_3, stream3_2),
        u128::MAX,
        "Node3 should refuse to read from stream with deleted source in chain"
    );
}
