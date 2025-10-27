use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};

use crate::{
    traceability::{infrastructure::naming::LocalizedResource, init_middleware},
    transport::{
        loopback::{spawn_loopback_middlewares, spawn_loopback_middlewares_with_entropy},
        nop::M2mNop,
    },
};

use super::fixtures::{FileMapping, StreamMapping};

#[tokio::test]
async fn integration_o2m_local_provenance() {
    // flowchart LR
    //     F(File) -->|1| P1[Process1]
    //     P1 -->|2| F

    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let (_, mut p2m_service, mut o2m_service) =
        init_middleware(String::new(), None, 0, M2mNop, false);

    let fd1 = FileMapping::new(1, 3, "/tmp/test1.txt");
    let fd2 = FileMapping::new(1, 4, "/tmp/test2.txt");

    local_enroll!(p2m_service, fd1);
    local_enroll!(p2m_service, fd2);

    read!(p2m_service, fd1);
    write!(p2m_service, fd2);

    assert_provenance!(
        o2m_service,
        fd1.file(),
        HashSet::from([LocalizedResource::new(String::new(), fd1.file())])
    );
    assert_provenance!(
        o2m_service,
        fd1.process(),
        HashSet::from([LocalizedResource::new(String::new(), fd1.process()), LocalizedResource::new(String::new(), fd1.file())])
    );
    assert_provenance!(
        o2m_service,
        fd2.process(),
        HashSet::from([LocalizedResource::new(String::new(), fd1.process()), LocalizedResource::new(String::new(), fd1.file())])
    );
    assert_provenance!(
        o2m_service,
        fd2.file(),
        HashSet::from([LocalizedResource::new(String::new(), fd2.file()), LocalizedResource::new(String::new(), fd1.process()), LocalizedResource::new(String::new(), fd1.file())])
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
        HashSet::from([LocalizedResource::new("10.0.0.1".to_string(), stream1.process())])
    );
    read!(p2m_2, stream2);
    assert_provenance!(
        o2m_2,
        stream2.process(),
        HashSet::from([LocalizedResource::new("10.0.0.1".to_string(), stream1.process()), LocalizedResource::new("10.0.0.2".to_string(), stream2.process())])
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
        HashSet::from([LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.file()), LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.process()), LocalizedResource::new("10.0.0.2".to_string(), stream2_3.process()), LocalizedResource::new("10.0.0.3".to_string(), stream3_2.process())])
    );

    assert_provenance!(
        o2m_2,
        stream2_3.process(), // P2on2 eq stream2_1.process()
        HashSet::from([LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.file()), LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.process()), LocalizedResource::new("10.0.0.2".to_string(), stream2_1.process())])
    );

    assert_provenance!(
        o2m_1,
        stream1_2.process(), // P1on1 eq fd1_1_1.process() | fd1_1_2.process()
        HashSet::from([LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.file()), LocalizedResource::new("10.0.0.1".to_string(), fd1_1_2.file()), LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.process())])
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
        spawn_loopback_middlewares_with_entropy(ips.clone(), 10, 100, 0, 0, 0).await.into_iter();

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
        HashSet::from([LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.file()), LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.process()), LocalizedResource::new("10.0.0.2".to_string(), stream2_3.process()), LocalizedResource::new("10.0.0.3".to_string(), stream3_2.process())])
    );

    assert_provenance!(
        o2m_2,
        stream2_3.process(), // P2on2 eq stream2_1.process()
        HashSet::from([LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.file()), LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.process()), LocalizedResource::new("10.0.0.2".to_string(), stream2_1.process())])
    );

    assert_provenance!(
        o2m_1,
        stream1_2.process(), // P1on1 eq fd1_1_1.process() | fd1_1_2.process()
        HashSet::from([LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.file()), LocalizedResource::new("10.0.0.1".to_string(), fd1_1_2.file()), LocalizedResource::new("10.0.0.1".to_string(), fd1_1_1.process())])
    );
}
