use std::io::Write;
use std::time::Duration;

use break_enforcer::Api;

fn main() {
    let mut api = Api::new().unwrap();

    loop {
        let idle = api.idle_since().unwrap();
        print!("\ruser has been idle for: {:?}   ", idle);
        std::io::stdout().flush().unwrap();

        std::thread::sleep(Duration::from_secs(1));
    }
}
