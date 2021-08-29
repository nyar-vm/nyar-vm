#[tokio::main]
fn main() {
    let state = AppState {};

    // create a `Router` that holds our state
    let app = Router::new()
        .route("/", get(handler))
        // provide the state so the router can access it
        .with_state(state);
}
