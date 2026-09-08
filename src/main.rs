#[tokio::main]
async fn main() -> Result<(), routepilot::AppError> {
    routepilot::run(std::env::args().skip(1).collect()).await
}
