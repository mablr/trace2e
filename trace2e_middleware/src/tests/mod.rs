#[macro_use]
mod fixtures;

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use fixtures::{FileMapping, StreamMapping};
use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};

use crate::{
    traceability::{
        api::{O2mRequest, O2mResponse, P2mRequest, P2mResponse},
        core::compliance::{ConfidentialityPolicy, Policy},
        init_middleware,
    },
    transport::{
        loopback::{spawn_loopback_middlewares, spawn_loopback_middlewares_with_entropy},
        nop::M2mNop,
    },
};

#[tokio::test]
async fn integration_init_middleware() {
    // flowchart LR
    //     P1[Process1] -->|1| F(File)
    //     P1[Process1] -->|2| S(Stream)
    //     F -->|3| P1
    //     S -->|4| P1

    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let (_, mut p2m_service, _) = init_middleware("10.0.0.1".to_string(), None, M2mNop);

    let file = FileMapping::new(1, 3, "/tmp/test.txt");
    let stream = StreamMapping::new(1, 4, "127.0.0.1:8080", "127.0.0.1:8081");

    local_enroll!(p2m_service, file);
    remote_enroll!(p2m_service, stream);

    write!(p2m_service, file);
    write!(p2m_service, stream);
    read!(p2m_service, file);
    read!(p2m_service, stream);
}

#[tokio::test]
async fn integration_spawn_loopback_middlewares() {
    // flowchart LR
    //     s1337on1@{ shape: h-cyl}
    //     s1338on2@{ shape: h-cyl}
    //     s1337on1["socket1337 on Node1"] --- s1338on2["socket1338 on Node2"]

    //     P1on1["Process1 on Node1"] -- 1 --> s1337on1
    //     s1338on2 -- 2 --> P1on2["Process1 on Node2"]
    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
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

    let stream1 = StreamMapping::new(1, 3, "10.0.0.1:1337", "10.0.0.2:1338");
    let stream2 = StreamMapping::new(1, 3, "10.0.0.2:1338", "10.0.0.1:1337");

    remote_enroll!(p2m_1, stream1);
    remote_enroll!(p2m_2, stream2);

    write!(p2m_1, stream1);
    read!(p2m_2, stream2);
}

#[tokio::test]
async fn integration_o2m_local_provenance() {
    // flowchart LR
    //     F(File) -->|1| P1[Process1]
    //     P1 -->|2| F

    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let (_, mut p2m_service, mut o2m_service) = init_middleware(String::new(), None, M2mNop);

    let fd1 = FileMapping::new(1, 3, "/tmp/test1.txt");
    let fd2 = FileMapping::new(1, 4, "/tmp/test2.txt");

    local_enroll!(p2m_service, fd1);
    local_enroll!(p2m_service, fd2);

    read!(p2m_service, fd1);
    write!(p2m_service, fd2);

    assert_provenance!(
        o2m_service,
        fd1.file(),
        HashMap::from([(String::new(), HashSet::from([fd1.file()]))])
    );
    assert_provenance!(
        o2m_service,
        fd1.process(),
        HashMap::from([(String::new(), HashSet::from([fd1.process(), fd1.file()]))])
    );
    assert_provenance!(
        o2m_service,
        fd2.process(),
        HashMap::from([(String::new(), HashSet::from([fd1.process(), fd1.file()]))])
    );
    assert_provenance!(
        o2m_service,
        fd2.file(),
        HashMap::from([(String::new(), HashSet::from([fd2.file(), fd1.process(), fd1.file()]))])
    );
}

#[tokio::test]
async fn integration_o2m_remote_provenance_basic() {
    // flowchart LR
    //     s1337on1["socket1337 on Node1"] --- s1338on2["socket1338 on Node2"]
    //     s1337on1@{ shape: h-cyl}
    //     s1338on2@{ shape: h-cyl}

    //     P1on1["Process1 on Node1"] -- 1 --> s1337on1
    //     s1338on2 -- 2 --> P1on2["Process1 on Node2"]
    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
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
    let (mut p2m_2, mut o2m_2) = middlewares.next().unwrap();

    let stream1 = StreamMapping::new(1, 3, "10.0.0.1:1337", "10.0.0.2:1338");
    let stream2 = StreamMapping::new(2, 3, "10.0.0.2:1338", "10.0.0.1:1337");

    remote_enroll!(p2m_1, stream1);
    remote_enroll!(p2m_2, stream2);

    write!(p2m_1, stream1);
    assert_provenance!(
        o2m_2,
        stream2.stream(),
        HashMap::from([("10.0.0.1".to_string(), HashSet::from([stream1.process()])),])
    );
    read!(p2m_2, stream2);
    assert_provenance!(
        o2m_2,
        stream2.process(),
        HashMap::from([
            ("10.0.0.1".to_string(), HashSet::from([stream1.process()])),
            ("10.0.0.2".to_string(), HashSet::from([stream2.process()]))
        ])
    );
}

#[tokio::test]
async fn integration_o2m_remote_provenance_complex() {
    // flowchart LR
    //     s1337on1["socket1337 on Node1"] --- s1338on2["socket1338 on Node2"]
    //     s1339on2["socket1339 on Node2"] --- s1340on3["socket1340 on Node3"]
    //     s1337on1@{ shape: h-cyl}
    //     s1338on2@{ shape: h-cyl}
    //     s1339on2@{ shape: h-cyl}
    //     s1340on3@{ shape: h-cyl}

    //     P1on1["Process1 on Node1"]
    //     P2on2["Process3 on Node2"]
    //     P3on3["Process3 on Node3"]

    //     F1_1_1[File1 opened by Process1@Node1]
    //     F1_1_2[File2 opened by Process1@Node1]

    //     F1_1_1 -- 1 --> P1on1
    //     P1on1 -- 2 --> s1337on1
    //     F1_1_2 -- 3 --> P1on1
    //     s1338on2 -- 4 --> P2on2
    //     P2on2 -- 5 --> s1339on2
    //     s1340on3 -- 6 --> P3on3

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
    let (mut p2m_2, mut o2m_2) = middlewares.next().unwrap();
    let (mut p2m_3, mut o2m_3) = middlewares.next().unwrap();

    let fd1_1_1 = FileMapping::new(1, 4, "/tmp/test1.txt");
    let fd1_1_2 = FileMapping::new(1, 5, "/tmp/test2.txt");

    local_enroll!(p2m_1, fd1_1_1);
    local_enroll!(p2m_1, fd1_1_2);

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
    read!(p2m_1, fd1_1_2);
    read!(p2m_2, stream2_1);
    write!(p2m_2, stream2_3);
    read!(p2m_3, stream3_2);

    assert_provenance!(
        o2m_3,
        stream3_2.process(), // P3on3
        HashMap::from([
            ("10.0.0.1".to_string(), HashSet::from([fd1_1_1.file(), fd1_1_1.process()])),
            ("10.0.0.2".to_string(), HashSet::from([stream2_3.process()])),
            ("10.0.0.3".to_string(), HashSet::from([stream3_2.process()]))
        ])
    );

    assert_provenance!(
        o2m_2,
        stream2_3.process(), // P2on2 eq stream2_1.process()
        HashMap::from([
            ("10.0.0.1".to_string(), HashSet::from([fd1_1_1.file(), fd1_1_1.process()])),
            ("10.0.0.2".to_string(), HashSet::from([stream2_1.process()])),
        ])
    );

    assert_provenance!(
        o2m_1,
        stream1_2.process(), // P1on1 eq fd1_1_1.process() | fd1_1_2.process()
        HashMap::from([(
            "10.0.0.1".to_string(),
            HashSet::from([fd1_1_1.file(), fd1_1_2.file(), fd1_1_1.process()])
        ),])
    );
}

#[tokio::test]
async fn integration_o2m_remote_provenance_complex_with_entropy() {
    // flowchart LR
    //     s1337on1["socket1337 on Node1"] --- s1338on2["socket1338 on Node2"]
    //     s1339on2["socket1339 on Node2"] --- s1340on3["socket1340 on Node3"]
    //     s1337on1@{ shape: h-cyl}
    //     s1338on2@{ shape: h-cyl}
    //     s1339on2@{ shape: h-cyl}
    //     s1340on3@{ shape: h-cyl}

    //     P1on1["Process1 on Node1"]
    //     P2on2["Process3 on Node2"]
    //     P3on3["Process3 on Node3"]

    //     F1_1_1[File1 opened by Process1@Node1]
    //     F1_1_2[File2 opened by Process1@Node1]

    //     F1_1_1 -- 1 --> P1on1
    //     P1on1 -- 2 --> s1337on1
    //     F1_1_2 -- 3 --> P1on1
    //     s1338on2 -- 4 --> P2on2
    //     P2on2 -- 5 --> s1339on2
    //     s1340on3 -- 6 --> P3on3

    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string(), "10.0.0.3".to_string()];
    let mut middlewares =
        spawn_loopback_middlewares_with_entropy(ips.clone(), 10, 100).await.into_iter();

    let (mut p2m_1, mut o2m_1) = middlewares.next().unwrap();
    let (mut p2m_2, mut o2m_2) = middlewares.next().unwrap();
    let (mut p2m_3, mut o2m_3) = middlewares.next().unwrap();

    let fd1_1_1 = FileMapping::new(1, 4, "/tmp/test1.txt");
    let fd1_1_2 = FileMapping::new(1, 5, "/tmp/test2.txt");

    local_enroll!(p2m_1, fd1_1_1);
    local_enroll!(p2m_1, fd1_1_2);

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
    read!(p2m_1, fd1_1_2);
    read!(p2m_2, stream2_1);
    write!(p2m_2, stream2_3);
    read!(p2m_3, stream3_2);

    assert_provenance!(
        o2m_3,
        stream3_2.process(), // P3on3
        HashMap::from([
            ("10.0.0.1".to_string(), HashSet::from([fd1_1_1.file(), fd1_1_1.process()])),
            ("10.0.0.2".to_string(), HashSet::from([stream2_3.process()])),
            ("10.0.0.3".to_string(), HashSet::from([stream3_2.process()]))
        ])
    );

    assert_provenance!(
        o2m_2,
        stream2_3.process(), // P2on2 eq stream2_1.process()
        HashMap::from([
            ("10.0.0.1".to_string(), HashSet::from([fd1_1_1.file(), fd1_1_1.process()])),
            ("10.0.0.2".to_string(), HashSet::from([stream2_1.process()])),
        ])
    );

    assert_provenance!(
        o2m_1,
        stream1_2.process(), // P1on1 eq fd1_1_1.process() | fd1_1_2.process()
        HashMap::from([(
            "10.0.0.1".to_string(),
            HashSet::from([fd1_1_1.file(), fd1_1_2.file(), fd1_1_1.process()])
        ),])
    );
}

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
    assert_policies!(o2m_1, HashSet::from([fd1_1_1.file()]), HashSet::from([Policy::default()]));
    set_policy!(
        o2m_1,
        fd1_1_1.file(),
        Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: Default::default(),
            deleted: false,
        }
    );
    assert_policies!(
        o2m_1,
        HashSet::from([fd1_1_1.file()]),
        HashSet::from([Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: Default::default(),
            deleted: false,
        }])
    );

    // This must be refused because the file1 is now private
    assert_eq!(write_request!(p2m_3, fd3_3_2), u128::MAX);

    // Set the policy for the file1 to make it public again
    set_policy!(o2m_1, fd1_1_1.file(), Policy::default());

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
    set_policy!(
        o2m_3,
        fd3_3_2.file(),
        Policy { confidentiality: Default::default(), integrity: 5, deleted: false }
    );

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
    set_policy!(o2m_3, fd3_3_2.file(), Policy::default());

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
    assert_policies!(o2m_1, HashSet::from([fd1_1_1.file()]), HashSet::from([Policy::default()]));

    // Mark the source file as deleted
    set_policy!(
        o2m_1,
        fd1_1_1.file(),
        Policy {
            confidentiality: Default::default(),
            integrity: Default::default(),
            deleted: true,
        }
    );

    // Verify the source file is now marked as deleted
    assert_policies!(
        o2m_1,
        HashSet::from([fd1_1_1.file()]),
        HashSet::from([Policy {
            confidentiality: Default::default(),
            integrity: Default::default(),
            deleted: true,
        }])
    );

    // This must be refused because the source file is now deleted
    // Any data flow involving deleted resources should be blocked
    assert_eq!(write_request!(p2m_2, fd2_2_1), u128::MAX);
    assert_eq!(read_request!(p2m_2, fd2_2_1), u128::MAX);
    assert_eq!(write_request!(p2m_1, fd3_3_1), u128::MAX);

    // Test that we cannot modify a deleted resource's policy
    // The set_policy call should have no effect (returns None for deleted resources)
    let _ = o2m_1
        .call(crate::traceability::api::O2mRequest::SetPolicy {
            resource: fd1_1_1.file(),
            policy: Policy::default(),
        })
        .await;

    // The policy should remain deleted (unchanged)
    assert_policies!(
        o2m_1,
        HashSet::from([fd1_1_1.file()]),
        HashSet::from([Policy {
            confidentiality: Default::default(),
            integrity: Default::default(),
            deleted: true,
        }])
    );
}
