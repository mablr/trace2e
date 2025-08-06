#[macro_use]
mod fixtures;

use fixtures::{FileMapping, StreamMapping};

use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};

use crate::{
    traceability::{
        api::{O2mRequest, O2mResponse, P2mRequest, P2mResponse},
        init_middleware,
    },
    transport::{loopback::spawn_loopback_middlewares, nop::M2mNop},
};

#[tokio::test]
async fn integration_init_middleware() {
    let (_, mut p2m_service, _) = init_middleware(None, M2mNop::default());

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
    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
    let mut middlewares = spawn_loopback_middlewares(ips.clone())
        .await
        .into_iter()
        .map(|(p2m, o2m)| {
            (
                ServiceBuilder::new()
                    .layer(TimeoutLayer::new(Duration::from_millis(1)))
                    .service(p2m),
                o2m,
            )
        })
        .collect::<VecDeque<_>>();

    let (mut p2m_1, _) = middlewares.pop_front().unwrap();
    let (mut p2m_2, _) = middlewares.pop_front().unwrap();

    let stream1 = StreamMapping::new(1, 3, "10.0.0.1:1337", "10.0.0.2:1338");
    let stream2 = StreamMapping::new(1, 3, "10.0.0.2:1338", "10.0.0.1:1337");

    remote_enroll!(p2m_1, stream1);
    remote_enroll!(p2m_2, stream2);

    write!(p2m_1, stream1);
    read!(p2m_2, stream2);
}

#[tokio::test]
async fn integration_o2m_local_provenance() {
    let (_, mut p2m_service, mut o2m_service) = init_middleware(None, M2mNop::default());

    let fd1 = FileMapping::new(1, 3, "/tmp/test1.txt");
    let fd2 = FileMapping::new(1, 4, "/tmp/test2.txt");

    local_enroll!(p2m_service, fd1);
    local_enroll!(p2m_service, fd2);

    read!(p2m_service, fd1);
    write!(p2m_service, fd2);

    assert_local_provenance!(o2m_service, fd1.file(), HashSet::from([fd1.file()]));
    assert_local_provenance!(
        o2m_service,
        fd1.process(),
        HashSet::from([fd1.process(), fd1.file()])
    );
    assert_local_provenance!(
        o2m_service,
        fd2.process(),
        HashSet::from([fd1.process(), fd1.file()])
    );
    assert_local_provenance!(
        o2m_service,
        fd2.file(),
        HashSet::from([fd2.file(), fd1.process(), fd1.file()])
    );
}
