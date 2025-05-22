use tower::{ServiceBuilder, filter::FilterLayer};

use super::*;

#[tokio::test]
async fn unit_traceability_provenance_service() {
    let mut provenance_service = ProvenanceService::default();
    let req = P2mRequest::LocalEnroll {
        pid: 1,
        fd: 1,
        path: "test".to_string(),
    };
    let res = provenance_service.call(req).await.unwrap();
    assert_eq!(res, P2mResponse::Enrolled);
}

#[tokio::test]
async fn unit_traceability_provenance_service_p2m_validator() {
    let validator = P2mValidator::default();
    let mut provenance_service = ServiceBuilder::new()
        .layer(FilterLayer::new(validator))
        .service(ProvenanceService::default());

    assert_eq!(
        provenance_service
            .call(P2mRequest::LocalEnroll {
                pid: 1,
                fd: 1,
                path: "test".to_string()
            })
            .await
            .unwrap(),
        P2mResponse::Enrolled
    );

    assert_eq!(
        provenance_service
            .call(P2mRequest::RemoteEnroll {
                pid: 1,
                local_socket: "127.0.0.1:8080".to_string(),
                peer_socket: "127.0.0.1:8081".to_string(),
                fd: 1
            })
            .await
            .unwrap(),
        P2mResponse::Enrolled
    );

    assert_eq!(
        provenance_service
            .call(P2mRequest::IoRequest {
                pid: 1,
                fd: 1,
                flow_type: FlowType::Read
            })
            .await
            .unwrap(),
        P2mResponse::Enrolled
    );

    assert_eq!(
        provenance_service
            .call(P2mRequest::IoReport {
                pid: 1,
                fd: 1,
                flow_id: 1,
                io_result: true
            })
            .await
            .unwrap(),
        P2mResponse::Enrolled
    );

    assert!(
        provenance_service
            .check(P2mRequest::LocalEnroll {
                pid: 0,
                fd: 1,
                path: "test".to_string()
            })
            .is_err(),
    );

    assert!(
        provenance_service
            .call(P2mRequest::LocalEnroll {
                pid: 0,
                fd: 1,
                path: "test".to_string()
            }) // pid 0 is invalid
            .await
            .is_err(),
    );

    assert!(
        provenance_service
            .call(P2mRequest::RemoteEnroll {
                pid: 1,
                local_socket: "bad_socket".to_string(),
                peer_socket: "bad_socket".to_string(),
                fd: 1
            })
            .await
            .is_err(),
    );
}

#[tokio::test]
async fn unit_traceability_resource_service() {
    let file = Identifier {
        node: "localhost".to_string(),
        variant: ResourceVariant::Fd(Fd::File(File {
            path: "/tmp/test".to_string(),
        })),
    };

    let stream = Identifier {
        node: "localhost".to_string(),
        variant: ResourceVariant::Fd(Fd::Stream(Stream {
            local_socket: "127.0.0.1:8080".to_string(),
            peer_socket: "127.0.0.1:8081".to_string(),
        })),
    };

    let process = Identifier {
        node: "localhost".to_string(),
        variant: ResourceVariant::Process(message::Process {
            pid: 1,
            starttime: 1,
            exe_path: "/tmp/test".to_string(),
        }),
    };

    let mut file_service = ResourceService::new(file.clone());
    assert_eq!(
        file_service.call(ResourceRequest::GetProv).await.unwrap(),
        ResourceResponse::Prov(vec![file])
    );

    let mut stream_service = ResourceService::new(stream.clone());
    assert_eq!(
        stream_service.call(ResourceRequest::GetProv).await.unwrap(),
        ResourceResponse::Prov(vec![stream])
    );

    let mut process_service = ResourceService::new(process.clone());
    assert_eq!(
        process_service
            .call(ResourceRequest::GetProv)
            .await
            .unwrap(),
        ResourceResponse::Prov(vec![process])
    );
}

#[tokio::test]
async fn unit_traceability_resource_service_resource_validator() {
    let file = Identifier {
        node: "localhost".to_string(),
        variant: ResourceVariant::Fd(Fd::File(File {
            path: "/tmp/test".to_string(),
        })),
    };

    let stream = Identifier {
        node: "localhost".to_string(),
        variant: ResourceVariant::Fd(Fd::Stream(Stream {
            local_socket: "127.0.0.1:8080".to_string(),
            peer_socket: "127.0.0.1:8081".to_string(),
        })),
    };

    let process = Identifier {
        node: "localhost".to_string(),
        variant: ResourceVariant::Process(message::Process {
            pid: 1,
            starttime: 1,
            exe_path: "/tmp/test".to_string(),
        }),
    };

    let mut file_service = ResourceService::new(file.clone());
    let mut stream_service = ResourceService::new(stream.clone());
    let mut process_service = ResourceService::new(process.clone());

    let file_prov_response = file_service.call(ResourceRequest::GetProv).await.unwrap();
    assert_eq!(
        file_prov_response,
        ResourceResponse::Prov(vec![file.clone()])
    );

    match file_prov_response {
        ResourceResponse::Prov(prov) => {
            process_service
                .call(ResourceRequest::UpdateProv(prov))
                .await
                .unwrap();
            let process_prov_response = process_service
                .call(ResourceRequest::GetProv)
                .await
                .unwrap();
            assert_eq!(
                process_prov_response,
                ResourceResponse::Prov(vec![process.clone(), file.clone()])
            );

            match process_prov_response {
                ResourceResponse::Prov(prov) => {
                    stream_service
                        .call(ResourceRequest::UpdateProv(prov))
                        .await
                        .unwrap();
                    let stream_prov_response =
                        stream_service.call(ResourceRequest::GetProv).await.unwrap();
                    assert_eq!(
                        stream_prov_response,
                        ResourceResponse::Prov(vec![stream.clone(), process.clone(), file.clone()])
                    );
                }
                _ => panic!("Unexpected Prov response for process"),
            }
        }
        _ => panic!("Unexpected Prov response for file"),
    }
}
