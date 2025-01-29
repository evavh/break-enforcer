use break_enforcer::ReconnectingApi;

fn main() {
    color_eyre::install().expect("Only called once");
    tracing_subscriber::fmt().init();

    let mut api = ReconnectingApi::new().subscribe();

    loop {
        let update = api.recv_update();
        println!("state changed: {update:?}");
    }
}
