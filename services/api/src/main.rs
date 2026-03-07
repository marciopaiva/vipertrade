use warp::Filter;

#[tokio::main]
async fn main() {
    let hello = warp::path::end().map(|| "Hello, ViperTrade API!");
    let health = warp::path("health").map(|| warp::reply::json(&"OK"));

    let routes = hello.or(health);

    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await;
}
