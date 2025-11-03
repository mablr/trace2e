use super::fixtures::{FileMapping, StreamMapping};
use crate::{
    traceability::services::consent::Destination,
    transport::loopback::spawn_loopback_middlewares,
};
use std::time::Duration;
use tokio::time::timeout;
use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};
#[cfg(feature = "trace2e_tracing")]
use tracing::info;

#[tokio::test]
async fn integration_consent_notification_local_and_remote_io() {
    // Simple test: Enable consent on a file, trigger IO, and verify notifications are sent
    // Consent decisions are made automatically via background task
    //
    // flowchart LR
    //     F1_1_1["File1 opened by Process1@Node1"] -- 1 --> P1on1["Process1 on Node1"]
    //     P1on1 -- 2 --> s1337on1["socket1337 on Node1"]
    //     s1337on1 --- s1338on2["socket1338 on Node2"]
    //     policy0(["Enable Consent"]) -. 0 .- F1_1_1

    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();

    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
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

    let fd1_1_1 = FileMapping::new(1, 4, "/tmp/test1.txt", "10.0.0.1".to_string());

    local_enroll!(p2m_1, fd1_1_1);

    let stream1_2 = StreamMapping::new(1, 3, "10.0.0.1:1337", "10.0.0.2:1338");
    let stream2_1 = StreamMapping::new(2, 3, "10.0.0.2:1338", "10.0.0.1:1337");

    remote_enroll!(p2m_1, stream1_2);
    remote_enroll!(p2m_2, stream2_1);

    // Enable consent enforcement on the file and get notification channel
    let mut notifications = enforce_consent!(o2m_1, fd1_1_1.file());

    // Spawn a task to receive and track consent notifications
    let fd1_file = fd1_1_1.file();
    let mut o2m_consent = o2m_1.clone();
    let notifications_task = tokio::spawn(async move {
        let mut count = 0;
        let mut destinations = Vec::new();

        // Wait for notifications with timeout
        while let Ok(Ok(destination)) =
            timeout(Duration::from_millis(50), notifications.recv()).await
        {
            destinations.push(destination.clone());
            count += 1;

            // Auto-grant consent for each notification
            set_consent_decision!(o2m_consent, fd1_file.clone(), destination, true);
        }

        (count, destinations)
    });

    // Trigger local read - this should generate a consent notification
    read!(p2m_1, fd1_1_1);

    // Write to remote stream - this should generate another consent notification
    write!(p2m_1, stream1_2);

    // Wait a bit for notifications to be processed
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Check the notifications we received
    let (notification_count, destinations) = notifications_task.await.unwrap();

    // We should have received 2 notifications
    assert!(
        notification_count == 2,
        "Expected 2 consent notifications, got {}",
        notification_count
    );

    // Verify the destinations are the expected ones
    assert_eq!(
        destinations[0],
        Destination::Resource {
            resource: fd1_1_1.process(),
            parent: Some(Box::new(Destination::Node("10.0.0.1".to_string())))
        }
    );
    assert_eq!(
        destinations[1],
        Destination::Resource {
            resource: stream2_1.stream(),
            parent: Some(Box::new(Destination::Node("10.0.0.2".to_string())))
        }
    );
}

#[tokio::test]
async fn integration_consent_decision_on_remote_io() {
    // Test: Enable consent on a file on node1, perform IO operations,
    // handle consent notifications, and verify data flow completes successfully
    //
    // This test demonstrates consent enforcement for data flowing from a file
    // on node1 to a stream destination on node2.

    #[cfg(feature = "trace2e_tracing")]
    crate::trace2e_tracing::init();

    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
    let mut middlewares =
        spawn_loopback_middlewares(ips.clone()).await.into_iter().map(|(p2m, o2m)| {
            (
                ServiceBuilder::new()
                    .layer(TimeoutLayer::new(Duration::from_millis(500)))
                    .service(p2m),
                o2m,
            )
        });

    let (mut p2m_1, mut o2m_1) = middlewares.next().unwrap();
    let (mut p2m_2, _) = middlewares.next().unwrap();

    let fd1_1_1 = FileMapping::new(1, 4, "/tmp/source.txt", "10.0.0.1".to_string());

    local_enroll!(p2m_1, fd1_1_1);

    let stream1_2 = StreamMapping::new(1, 3, "10.0.0.1:1337", "10.0.0.2:1338");
    let stream2_1 = StreamMapping::new(2, 3, "10.0.0.2:1338", "10.0.0.1:1337");

    remote_enroll!(p2m_1, stream1_2);
    remote_enroll!(p2m_2, stream2_1);

    // Enable consent enforcement on the source file and get notification channel
    let mut notifications = enforce_consent!(o2m_1, fd1_1_1.file());

    // Spawn a background task to handle consent requests automatically
    let fd1_file = fd1_1_1.file();
    let mut o2m_consent = o2m_1.clone();
    let consent_task = tokio::spawn(async move {
        let mut granted_destinations = Vec::new();

        // Wait for notifications and auto-grant consent
        while let Ok(Ok(destination)) =
            timeout(Duration::from_millis(150), notifications.recv()).await
        {
            #[cfg(feature = "trace2e_tracing")]
            info!("Received consent notification for destination: {:?}", destination);
            granted_destinations.push(destination.clone());

            // Auto-grant consent immediately
            set_consent_decision!(o2m_consent, fd1_file.clone(), destination, true);
        }

        granted_destinations
    });

    // Give the consent handler time to start listening
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Perform IO operations that will trigger consent notifications
    read!(p2m_1, fd1_1_1);
    write!(p2m_1, stream1_2);

    // Wait for consent processing
    tokio::time::sleep(Duration::from_millis(50)).await;

    let granted_destinations = consent_task.await.unwrap();

    // Verify we received and granted consent requests
    assert!(!granted_destinations.is_empty(), "Expected at least one consent request");

    // Verify one of the destinations is for node2 or the stream
    let has_remote_dest = granted_destinations.iter().any(|dest| match dest {
        Destination::Resource { resource, .. } => resource == &stream2_1.stream(),
        Destination::Node(node_id) => node_id == "10.0.0.2",
    });
    assert!(has_remote_dest, "Expected consent for remote destination");
}
