use std::time::Duration;

use break_enforcer;
use tokio::time::sleep;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    loop {
        sleep(Duration::from_secs(1)).await;

        if break_enforcer::is_active().await {
            println!("it is active");
        } else {
            println!("it is not active :(");
        }
    }
}
