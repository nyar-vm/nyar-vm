use axum::http::StatusCode;

pub struct JsonResponse<T> {
    code: StatusCode,
    message: String,
    data: T,
}

impl From<T> for JsonResponse<T> {
    fn from(data: T) -> Self {
        Self { code: StatusCode::OK, message: String::new(), data }
    }
}
