use crate::cli::StatusArgs;
use break_enforcer::ReconnectingApi;
use color_eyre::eyre::WrapErr;
use color_eyre::Section;

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

pub fn run(
    StatusArgs {
        update_period,
        use_json,
    }: StatusArgs,
) -> color_eyre::Result<()> {
    let mut api = ReconnectingApi::new();
    let Some(period) = update_period else {
        let msg = api
            .status()
            .wrap_err("Error requesting status message")
            .suggestion(
                "Is break-enforcer running and is it running with its tcp api \
                enabled? (use --tcp-api)",
            )?;
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
