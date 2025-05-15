use trace2e_middleware::grpc_proto::{
    resource::Variant, trace2e_client::Trace2eClient, Ack, File, Req, Resource,
};

#[tokio::test]
async fn integration_client_enable_local_confidentiality() -> Result<(), Box<dyn std::error::Error>>
{
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;

    // User request
    let resource = tonic::Request::new(Resource {
        variant: Some(Variant::File(File {
            path: "/dev/null".to_string(),
        })),
    });
    let compliance_action = client
        .user_enable_local_confidentiality(resource)
        .await?
        .into_inner();
    assert_eq!(compliance_action, Ack {});

    Ok(())
}

#[tokio::test]
async fn integration_client_disable_local_confidentiality() -> Result<(), Box<dyn std::error::Error>>
{
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;

    // User request
    let resource = tonic::Request::new(Resource {
        variant: Some(Variant::File(File {
            path: "/dev/null".to_string(),
        })),
    });
    let compliance_action = client
        .user_disable_local_confidentiality(resource)
        .await?
        .into_inner();
    assert_eq!(compliance_action, Ack {});

    Ok(())
}

#[tokio::test]
async fn integration_client_print_db() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;

    // User request
    let req = tonic::Request::new(Req {});
    let print_db = client.user_print_db(req).await?.into_inner();
    assert_eq!(print_db, Ack {});

    Ok(())
}
