use std::time::Duration;

use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};

use crate::{
    traceability::init_middleware,
    transport::{loopback::spawn_loopback_middlewares, nop::M2mNop},
};

use super::fixtures::{FileMapping, StreamMapping};

#[tokio::test]
async fn integration_init_middleware() {
    // flowchart LR
    //     P1[Process1] -->|1| F(File)
    //     P1[Process1] -->|2| S(Stream)
    //     F -->|3| P1
    //     S -->|4| P1
    crate::trace2e_tracing::init();
    let (_, mut p2m_service, _) = init_middleware("10.0.0.1".to_string(), None, 0, M2mNop, false);

    let file = FileMapping::new(1, 3, "/tmp/test.txt", "10.0.0.1".to_string());
    let stream = StreamMapping::new(1, 4, "10.0.0.1:8080", "10.0.0.2:8081");

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
