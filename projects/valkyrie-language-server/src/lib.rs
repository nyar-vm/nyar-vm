use axum::{extract::State, routing::get, Router};

// the application state
//
// here you can put configuration, database connection pools, or whatever
// state you need
//
// see "When states need to implement `Clone`" for more details on why we need
// `#[derive(Clone)]` here.
#[derive(Clone)]
struct AppState {}

async fn handler(
    // access the state via the `State` extractor
    // extracting a state of the wrong type results in a compile error
    State(state): State<AppState>,
) {
    // use `state`...
}
