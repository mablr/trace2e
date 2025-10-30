use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};

use crate::{
    traceability::api::types::O2mRequest,
    traceability::services::compliance::{ConfidentialityPolicy, DeletionPolicy, Policy},
    transport::loopback::spawn_loopback_middlewares,
};

use super::fixtures::{FileMapping, StreamMapping};

#[tokio::test]
async fn integration_o2m_remote_confidentiality_enforcement() {
    // flowchart LR
    //     s1337on1["socket1337 on Node1"] --- s1338on2["socket1338 on Node2"]
    //     s1339on2["socket1339 on Node2"] --- s1340on3["socket1340 on Node3"]
    //     F1_1_1["File1 opened by Process1@Node1"] -- 1 --> P1on1["Process1 on Node1"]
    //     P1on1 -- 2 --> s1337on1
    //     s1338on2 -- 3 --> P2on2["Process3 on Node2"]
    //     P2on2 -- 4 --> s1339on2
    //     s1340on3 -- 5 --> P3on3["Process3 on Node3"]
    //     policy0(["Set Private"]) -. 6 .- F1_1_1
    //     P3on3 -- 7 --x F3_3_2["File2 opened by Process3@Node3"]
    //     policy1(["Set Public"]) -. 8 .- F1_1_1
    //     P3on3 -- 9 --> F3_3_2

    //     s1337on1@{ shape: h-cyl}
    //     s1338on2@{ shape: h-cyl}
    //     s1339on2@{ shape: h-cyl}
    //     s1340on3@{ shape: h-cyl}

    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string(), "10.0.0.3".to_string()];
    let mut middlewares =
        spawn_loopback_middlewares(ips.clone()).await.into_iter().map(|(p2m, o2m)| {
            (
                ServiceBuilder::new()
                    .layer(TimeoutLayer::new(Duration::from_millis(1)))
                    .service(p2m),
                o2m,
            )
        });

    let (mut p2m_1, mut o2m_1) = middlewares.next().unwrap();
    let (mut p2m_2, _) = middlewares.next().unwrap();
    let (mut p2m_3, _) = middlewares.next().unwrap();

    let fd1_1_1 = FileMapping::new(1, 4, "/tmp/test1.txt");
    let fd3_3_2 = FileMapping::new(3, 4, "/tmp/test2.txt");

    local_enroll!(p2m_1, fd1_1_1);
    local_enroll!(p2m_3, fd3_3_2);

    let stream1_2 = StreamMapping::new(1, 3, "10.0.0.1:1337", "10.0.0.2:1338");
    let stream2_1 = StreamMapping::new(2, 3, "10.0.0.2:1338", "10.0.0.1:1337");
    let stream2_3 = StreamMapping::new(2, 4, "10.0.0.2:1339", "10.0.0.3:1340");
    let stream3_2 = StreamMapping::new(3, 3, "10.0.0.3:1340", "10.0.0.2:1339");

    remote_enroll!(p2m_1, stream1_2);
    remote_enroll!(p2m_2, stream2_1);
    remote_enroll!(p2m_2, stream2_3);
    remote_enroll!(p2m_3, stream3_2);

    read!(p2m_1, fd1_1_1);
    write!(p2m_1, stream1_2);
    read!(p2m_2, stream2_1);
    write!(p2m_2, stream2_3);
    read!(p2m_3, stream3_2);

    // Set the policy for the file1 to make it private
    assert_policies!(
        o2m_1,
        HashSet::from([fd1_1_1.file()]),
        HashMap::from([(fd1_1_1.localized_file(), Policy::default())])
    );
    set_confidentiality!(o2m_1, fd1_1_1.file(), ConfidentialityPolicy::Secret);
    assert_policies!(
        o2m_1,
        HashSet::from([fd1_1_1.file()]),
        HashMap::from([(
            fd1_1_1.localized_file(),
            Policy::new(
                ConfidentialityPolicy::Secret,
                Default::default(),
                DeletionPolicy::NotDeleted,
                false
            )
        )])
    );

    // This must be refused because the file1 is now private
    assert_eq!(write_request!(p2m_3, fd3_3_2), u128::MAX);

    // Set the policy for the file1 to make it public again
    set_confidentiality!(o2m_1, fd1_1_1.file(), ConfidentialityPolicy::Public);

    // This must be granted because the file1 is now public
    // assert_eq!(write_request!(p2m_3, fd3_3_2), 0);
    write!(p2m_3, fd3_3_2);
}

#[tokio::test]
async fn integration_o2m_remote_integrity_enforcement() {
    // flowchart LR
    //     s1337on1["socket1337 on Node1"] --- s1338on2["socket1338 on Node2"]
    //     s1339on2["socket1339 on Node2"] --- s1340on3["socket1340 on Node3"]
    //     F1_1_1["File1 opened by Process1@Node1"] -- 2 --> P1on1["Process1 on Node1"]
    //     P1on1 -- 3 --> s1337on1
    //     s1338on2 -- 4 --> P2on2["Process3 on Node2"]
    //     P2on2 -- 5 --> s1339on2
    //     s1340on3 -- 6 --> P3on3["Process3 on Node3"]
    //     policy0(["Set High Integrity"]) -. 1 .- F3_3_2
    //     P3on3 -- 7 --x F3_3_2["File2 opened by Process3@Node3"]
    //     policy1(["Set Low Integrity"]) -. 8 .- F3_3_2
    //     P3on3 -- 9 --> F3_3_2

    //     s1337on1@{ shape: h-cyl}
    //     s1338on2@{ shape: h-cyl}
    //     s1339on2@{ shape: h-cyl}
    //     s1340on3@{ shape: h-cyl}

    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string(), "10.0.0.3".to_string()];
    let mut middlewares =
        spawn_loopback_middlewares(ips.clone()).await.into_iter().map(|(p2m, o2m)| {
            (
                ServiceBuilder::new()
                    .layer(TimeoutLayer::new(Duration::from_millis(1)))
                    .service(p2m),
                o2m,
            )
        });

    let (mut p2m_1, _) = middlewares.next().unwrap();
    let (mut p2m_2, _) = middlewares.next().unwrap();
    let (mut p2m_3, mut o2m_3) = middlewares.next().unwrap();

    let fd1_1_1 = FileMapping::new(1, 4, "/tmp/test1.txt");
    let fd3_3_2 = FileMapping::new(3, 4, "/tmp/test2.txt");

    // Set the destination's integrity requirement to 5
    set_integrity!(o2m_3, fd3_3_2.file(), 5);

    local_enroll!(p2m_1, fd1_1_1);
    local_enroll!(p2m_3, fd3_3_2);

    let stream1_2 = StreamMapping::new(1, 3, "10.0.0.1:1337", "10.0.0.2:1338");
    let stream2_1 = StreamMapping::new(2, 3, "10.0.0.2:1338", "10.0.0.1:1337");
    let stream2_3 = StreamMapping::new(2, 4, "10.0.0.2:1339", "10.0.0.3:1340");
    let stream3_2 = StreamMapping::new(3, 3, "10.0.0.3:1340", "10.0.0.2:1339");

    remote_enroll!(p2m_1, stream1_2);
    remote_enroll!(p2m_2, stream2_1);
    remote_enroll!(p2m_2, stream2_3);
    remote_enroll!(p2m_3, stream3_2);

    read!(p2m_1, fd1_1_1);
    write!(p2m_1, stream1_2);
    read!(p2m_2, stream2_1);
    write!(p2m_2, stream2_3);
    read!(p2m_3, stream3_2);

    // This must be refused because the source integrity (1) is less than destination integrity (5)
    assert_eq!(write_request!(p2m_3, fd3_3_2), u128::MAX);

    // Lower the destination's integrity requirement to allow the write
    set_integrity!(o2m_3, fd3_3_2.file(), 0);

    // This must be granted because the destination now accepts any integrity level
    write!(p2m_3, fd3_3_2);
}

#[tokio::test]
async fn integration_o2m_remote_delete_policy_enforcement() {
    // flowchart LR
    //     s1337on1["socket1337 on Node1"] --- s1338on2["socket1338 on Node2"]
    //     F1_1_1["File1 opened by Process1@Node1"] -- 1 --> P1on1["Process1 on Node1"]
    //     P1on1 -- 2 --> s1337on1
    //     s1338on2 -- 3 --> P2on2["Process2 on Node2"]
    //     P2on2 -- 4 --> F2_2_1["File2 opened by Process2@Node2"]
    //     policy0(["Mark as Deleted"]) -. 5 .- F1_1_1
    //     P2on2 -- 6 --x F2_2_1
    //     F2_2_1 -- 7 --x P2on2
    //     P3on1["Process3 on Node1"] -- 8 --x F1_1_1

    //     s1337on1@{ shape: h-cyl}
    //     s1338on2@{ shape: h-cyl}

    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
    let mut middlewares =
        spawn_loopback_middlewares(ips.clone()).await.into_iter().map(|(p2m, o2m)| {
            (
                ServiceBuilder::new()
                    .layer(TimeoutLayer::new(Duration::from_millis(10)))
                    .service(p2m),
                o2m,
            )
        });

    let (mut p2m_1, mut o2m_1) = middlewares.next().unwrap();
    let (mut p2m_2, _) = middlewares.next().unwrap();

    let fd1_1_1 = FileMapping::new(1, 4, "/tmp/source.txt");
    let fd3_3_1 = FileMapping::new(3, 4, "/tmp/source.txt");
    let fd2_2_1 = FileMapping::new(2, 4, "/tmp/destination.txt");

    local_enroll!(p2m_1, fd1_1_1);
    local_enroll!(p2m_1, fd3_3_1);
    local_enroll!(p2m_2, fd2_2_1);

    let stream1_2 = StreamMapping::new(1, 3, "10.0.0.1:1337", "10.0.0.2:1338");
    let stream2_1 = StreamMapping::new(2, 3, "10.0.0.2:1338", "10.0.0.1:1337");

    remote_enroll!(p2m_1, stream1_2);
    remote_enroll!(p2m_2, stream2_1);

    // Establish initial data flow from source file to destination
    read!(p2m_1, fd1_1_1);
    write!(p2m_1, stream1_2);
    read!(p2m_2, stream2_1);

    // Initially, writing to destination should work
    write!(p2m_2, fd2_2_1);

    // Verify the source file has default policy
    assert_policies!(
        o2m_1,
        HashSet::from([fd1_1_1.file()]),
        HashMap::from([(fd1_1_1.localized_file(), Policy::default())])
    );

    // Mark the source file as deleted
    set_deleted!(o2m_1, fd1_1_1.file());

    // Verify the source file is now marked as deleted
    assert_policies!(
        o2m_1,
        HashSet::from([fd1_1_1.file()]),
        HashMap::from([(
            fd1_1_1.localized_file(),
            Policy::new(Default::default(), Default::default(), DeletionPolicy::Pending, false)
        )])
    );

    // This must be refused because the source file is now deleted
    // Any data flow involving deleted resources should be blocked
    assert_eq!(write_request!(p2m_2, fd2_2_1), u128::MAX);
    assert_eq!(read_request!(p2m_2, fd2_2_1), u128::MAX);
    assert_eq!(write_request!(p2m_1, fd3_3_1), u128::MAX);

    // Test that we cannot modify a deleted resource's policy
    // The set_policy call should have no effect (returns None for deleted resources)
    let _ = o2m_1
        .call(O2mRequest::SetPolicy { resource: fd1_1_1.file(), policy: Policy::default() })
        .await;

    // The policy should remain deleted (unchanged)
    assert_policies!(
        o2m_1,
        HashSet::from([fd1_1_1.file()]),
        HashMap::from([(
            fd1_1_1.localized_file(),
            Policy::new(Default::default(), Default::default(), DeletionPolicy::Pending, false)
        )])
    );
}
