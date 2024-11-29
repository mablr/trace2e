use trace2e_middleware::user_service::user::{
    resource::Variant, user_client::UserClient, Ack, File, Req, Resource,
};

#[tokio::test]
async fn integration_client_enable_local_confidentiality() -> Result<(), Box<dyn std::error::Error>>
{
    let mut client = UserClient::connect("http://[::1]:8080").await?;

    // User request
    let resource = tonic::Request::new(Resource {
        variant: Some(Variant::File(File {
            path: "/home/dan/sources/hyper/examples/send_file_index.html".to_string(),
        })),
    });
    let compliance_action = client
        .enable_local_confidentiality(resource)
        .await?
        .into_inner();
    assert_eq!(compliance_action, Ack {});

    Ok(())
}

#[tokio::test]
async fn integration_client_print_db() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = UserClient::connect("http://[::1]:8080").await?;

    // User request
    let req = tonic::Request::new(Req {});
    let print_db = client.print_db(req).await?.into_inner();
    assert_eq!(print_db, Ack {});

    Ok(())
}
