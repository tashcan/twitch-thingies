use std::error::Error;

async fn web_server() {
    use warp::Filter;

    let routes = warp::any().map(|| {
        eprintln!("Req");
        "Hello, World!"
    });
    warp::serve(routes)
        .run(([0, 0, 0, 0, 0, 0, 0, 0], 8081))
        .await;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    eprintln!("FUCK 1");
    web_server().await;
    eprintln!("FUCK 2");
    Ok(())
}
