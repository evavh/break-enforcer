use crate::cli::StatusArgs;
use break_enforcer::Api;
use color_eyre::eyre::WrapErr;

fn format_status(
    status: Result<String, break_enforcer::Error>,
    use_json: bool,
) -> String {
    match (status, use_json) {
        (Ok(msg), true) => format!("{{\"msg\": \"{msg}\"}}"),
        (Ok(msg), false) => msg,
        (Err(err), true) => format!("{{\"msg\": \"{err}\"}}"),
        (Err(err), false) => err.to_string(),
    }
}

#[derive(Default)]
enum ReconnectingApi {
    #[default]
    Disconnected,
    Connected(Api),
}

impl ReconnectingApi {
    fn new() -> Self {
        ReconnectingApi::Disconnected
    }

    fn status(&mut self) -> Result<String, break_enforcer::Error> {
        let placeholder = ReconnectingApi::default();
        let owned_self = core::mem::replace(self, placeholder);

        let mut api = match owned_self {
            ReconnectingApi::Disconnected => break_enforcer::Api::new()?,
            ReconnectingApi::Connected(api) => api,
        };

        match api.status() {
            Ok(status) => {
                *self = ReconnectingApi::Connected(api);
                Ok(status)
            }
            Err(e) => {
                *self = ReconnectingApi::Disconnected;
                Err(e)
            }
        }
    }
}

pub fn run(
    StatusArgs {
        update_period,
        use_json,
    }: StatusArgs,
) -> color_eyre::Result<()> {
    let mut api = ReconnectingApi::new();
    let Some(period) = update_period else {
        let msg = api.status().wrap_err("Error requesting status message")?;
        let output = format_status(Ok(msg), use_json);
        println!("{output}");
        return Ok(());
    };

    loop {
        let msg = api.status();
        let output = format_status(msg, use_json);
        println!("{output}");
        std::thread::sleep(period);
    }
}
