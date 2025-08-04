use std::time::Duration;

use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};

use crate::{
    traceability::{
        api::{P2mRequest, P2mResponse},
        init_middleware,
    },
    transport::{loopback::spawn_loopback_middlewares, nop::M2mNop},
};

#[tokio::test]
async fn integration_init_middleware() {
    let (_, mut p2m_service) = init_middleware(None, M2mNop::default());
    assert_eq!(
        p2m_service
            .call(P2mRequest::RemoteEnroll {
                pid: 1,
                fd: 3,
                local_socket: "127.0.0.1:8080".to_string(),
                peer_socket: "127.0.0.1:8081".to_string(),
            })
            .await
            .unwrap(),
        P2mResponse::Ack
    );
    let P2mResponse::Grant(flow_id) = p2m_service
        .call(P2mRequest::IoRequest {
            pid: 1,
            fd: 3,
            output: true,
        })
        .await
        .unwrap()
    else {
        panic!("Expected P2mResponse::Grant");
    };

    assert_eq!(
        p2m_service
            .call(P2mRequest::IoReport {
                pid: 1,
                fd: 3,
                grant_id: flow_id,
                result: true,
            })
            .await
            .unwrap(),
        P2mResponse::Ack
    );
}

#[tokio::test]
async fn integration_spawn_loopback_middlewares() {
    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
    let mut middlewares = spawn_loopback_middlewares(ips.clone())
        .await
        .into_iter()
        .map(|p2m| {
            ServiceBuilder::new()
                .layer(TimeoutLayer::new(Duration::from_millis(1)))
                .service(p2m)
        })
        .collect::<Vec<_>>();

    let mut p2m_2 = middlewares.pop().unwrap();
    let mut p2m_1 = middlewares.pop().unwrap();

    assert_eq!(
        p2m_1
            .call(P2mRequest::RemoteEnroll {
                pid: 1,
                fd: 3,
                local_socket: "10.0.0.1:1337".to_string(),
                peer_socket: "10.0.0.2:1338".to_string(),
            })
            .await
            .unwrap(),
        P2mResponse::Ack
    );

    assert_eq!(
        p2m_2
            .call(P2mRequest::RemoteEnroll {
                pid: 1,
                fd: 3,
                local_socket: "10.0.0.2:1338".to_string(),
                peer_socket: "10.0.0.1:1337".to_string(),
            })
            .await
            .unwrap(),
        P2mResponse::Ack
    );

    let P2mResponse::Grant(flow_id) = p2m_1
        .call(P2mRequest::IoRequest {
            pid: 1,
            fd: 3,
            output: true,
        })
        .await
        .unwrap()
    else {
        panic!("Expected P2mResponse::Grant");
    };

    assert_eq!(
        p2m_1
            .call(P2mRequest::IoReport {
                pid: 1,
                fd: 3,
                grant_id: flow_id,
                result: true,
            })
            .await
            .unwrap(),
        P2mResponse::Ack
    );
    let P2mResponse::Grant(flow_id) = p2m_2
        .call(P2mRequest::IoRequest {
            pid: 1,
            fd: 3,
            output: false,
        })
        .await
        .unwrap()
    else {
        panic!("Expected P2mResponse::Grant");
    };

    assert_eq!(
        p2m_2
            .call(P2mRequest::IoReport {
                pid: 1,
                fd: 3,
                grant_id: flow_id,
                result: true,
            })
            .await
            .unwrap(),
        P2mResponse::Ack
    );
}
