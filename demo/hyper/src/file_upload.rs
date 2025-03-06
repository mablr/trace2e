#![deny(warnings)]

use std::net::SocketAddr;

use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::{Incoming as IncomingBody, Bytes}, header, Method, Request, Response, StatusCode};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[path = "../benches/support/mod.rs"]
mod support;
use support::TokioIo;

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, GenericError>;
type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

static NOTFOUND: &[u8] = b"Not Found";
static OK: &[u8] = b"OK";


async fn api_upload_file(req: Request<IncomingBody>, path: &str) -> Result<Response<BoxBody>> {

    let file_name = path.split('/').last().unwrap();
    // Save file using the filename from the request
    let mut file = OpenOptions::new().write(true).create(true).open(file_name).await?;

    // Aggregate the body...
    let whole_body = req.collect().await?;
    let bytes = whole_body.to_bytes();

    // Write the bytes to the file
    file.write_all(&bytes).await?;

    // Read template file to a string
    let template_string = if let Ok(mut template_file) = File::open("template.html").await {
        let mut template_content = Vec::new();
        template_file.read_to_end(&mut template_content).await?;
        String::from_utf8(template_content).unwrap()
    } else {
        String::new()
    };
    // Replace the placeholder in the template with the file name
    let response_html = template_string.replace("{{file_name}}", file_name);
    
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .body(full(response_html))?;
    Ok(response)
}

async fn api_reset_template() -> Result<Response<BoxBody>> {
    let mut template_file = std::fs::File::create("template.html")?;
    
    std::io::Write::write_all(&mut template_file, b"<p> File '{{file_name}}' is successfully uploaded</p>")?;
    
    let response = Response::builder()
        .status(StatusCode::OK)
        .body(full(OK))?;
    Ok(response)
}

async fn router(req: Request<IncomingBody>) -> Result<Response<BoxBody>> {
    let path = req.uri().path().to_string();
    if req.method() == Method::POST && path.starts_with("/upload/") {
        api_upload_file(req, &path).await
    } else if req.method() == Method::POST && path == "/reset_template" {
        api_reset_template().await
    } else {
        // Return 404 not found response.
        Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(full(NOTFOUND))
                .unwrap())
    }
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let addr: SocketAddr = "0.0.0.0:1339".parse().unwrap();

    let listener = TcpListener::bind(&addr).await?;
    println!("Listening on http://{}", addr);
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            let service = service_fn(router);

            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}
