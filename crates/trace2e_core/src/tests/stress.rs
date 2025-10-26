use tower::Service;

use crate::{
    traceability::api::types::{P2mRequest, P2mResponse},
    transport::loopback::spawn_loopback_middlewares_with_enrolled_resources,
};

#[tokio::test]
async fn integration_p2m_stress_interference() {
    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let processes_count = 10;
    let files_per_process_count = 10;
    let streams_per_process_count = 10;

    // Pre-initialize P2M service with many processes and files
    let mut middlewares = spawn_loopback_middlewares_with_enrolled_resources(
        vec!["127.0.0.1".to_string(), "127.0.0.2".to_string()],
        processes_count,
        files_per_process_count,
        streams_per_process_count,
    )
    .await;

    let (p2m_service_0, _) = middlewares.pop_front().unwrap();
    let (p2m_service_1, _) = middlewares.pop_front().unwrap();

    // Spawn concurrent tasks that handle complete request-grant-report cycles
    let mut tasks = Vec::new();

    // Create interference patterns: multiple processes compete for same files
    for pid in 0..processes_count as i32 {
        for fd in 0..(files_per_process_count + streams_per_process_count) as i32 {
            let mut p2m_service_0_clone = p2m_service_0.clone();
            let mut p2m_service_1_clone = p2m_service_1.clone();
            let task = tokio::spawn(async move {
                // Complete I/O cycle: request -> grant -> report
                if let Ok(P2mResponse::Grant(grant_id)) = p2m_service_0_clone
                    .call(P2mRequest::IoRequest { pid, fd, output: (pid + fd) % 2 == 0 })
                    .await
                {
                    // Report completion
                    let _ = p2m_service_0_clone
                        .call(P2mRequest::IoReport { pid, fd, grant_id, result: true })
                        .await;
                }
                // Complete I/O cycle: request -> grant -> report
                if let Ok(P2mResponse::Grant(grant_id)) = p2m_service_1_clone
                    .call(P2mRequest::IoRequest { pid, fd, output: (pid + fd) % 2 == 0 })
                    .await
                {
                    // Report completion
                    let _ = p2m_service_1_clone
                        .call(P2mRequest::IoReport { pid, fd, grant_id, result: true })
                        .await;
                }
            });
            tasks.push(task);
        }
    }

    // Wait for all concurrent I/O operations to complete
    for task in tasks {
        let _ = task.await;
    }
}

#[tokio::test]
async fn integration_p2m_seq_stress_interference() {
    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();
    let processes_count = 10;
    let files_per_process_count = 10;
    let streams_per_process_count = 10;

    // Pre-initialize P2M service with many processes and files
    let mut middlewares = spawn_loopback_middlewares_with_enrolled_resources(
        vec!["127.0.0.1".to_string(), "127.0.0.2".to_string()],
        processes_count,
        files_per_process_count,
        streams_per_process_count,
    )
    .await;

    let (mut p2m_service_0, _) = middlewares.pop_front().unwrap();
    let (mut p2m_service_1, _) = middlewares.pop_front().unwrap();

    // Create interference patterns: multiple processes compete for same files
    for pid in 0..processes_count as i32 {
        for fd in 0..(files_per_process_count + streams_per_process_count) as i32 {
            // Complete I/O cycle: request -> grant -> report
            if let Ok(P2mResponse::Grant(grant_id)) = p2m_service_0
                .call(P2mRequest::IoRequest { pid, fd, output: (pid + fd) % 2 == 0 })
                .await
            {
                // Report completion
                let _ = p2m_service_0
                    .call(P2mRequest::IoReport { pid, fd, grant_id, result: true })
                    .await;
            }
            // Complete I/O cycle: request -> grant -> report
            if let Ok(P2mResponse::Grant(grant_id)) = p2m_service_1
                .call(P2mRequest::IoRequest { pid, fd, output: (pid + fd) % 2 == 0 })
                .await
            {
                // Report completion
                let _ = p2m_service_1
                    .call(P2mRequest::IoReport { pid, fd, grant_id, result: true })
                    .await;
            }
        }
    }
}
