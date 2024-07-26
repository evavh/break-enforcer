use std::io::Write;
use std::process::{Command, Stdio};

use color_eyre::eyre::{eyre, Context};
use color_eyre::{Result, Section};

struct User {
    id: String,
    name: String,
}

/// on the first failure this returns
fn all_users() -> Result<Vec<User>> {
    let users = Command::new("loginctl")
        .output()
        .wrap_err("could not run loginctl")?
        .stdout;
    let users = String::from_utf8(users).wrap_err("loginctl could not be parsed as utf8")?;
    users
        .lines()
        .filter(|x| x.starts_with(' '))
        .map(|x| x.split(' ').filter(|x| !x.is_empty()))
        .map(|mut x| {
            Ok(User {
                id: x
                    .nth(1)
                    .ok_or(eyre!("no user id in loginctl output"))?
                    .to_owned(),
                name: x
                    .next()
                    .ok_or(eyre!("no user name in loginctl output"))?
                    .to_owned(),
            })
        })
        .collect()
}

pub(crate) fn beep() -> Result<()> {
    let sound1 = include_bytes!("../../assets/new-notification-on-your-device-by-UNIVERSFIELD.wav");
    // let sound2 = include_bytes!("../../assets/notification-1-by-UNIVERSFIELD.wav");

    for User { id, name } in all_users().wrap_err("Could not get logged in users")? {
        let command = format!("sudo -u {name} XDG_RUNTIME_DIR=/run/user/{id} aplay");
        let aplay = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::piped())
            .spawn()
            .wrap_err("Could not spawn shell")
            .with_note(|| format!("as user: {id}:{name}"))?;
        let mut stdin = aplay.stdin.expect("is set to piped");
        stdin
            .write_all(sound1)
            .wrap_err("Could not pipe to aplay")?;
    }

    Ok(())
}

pub(crate) fn notify(text: &str) -> Result<()> {
    for User { id, name } in all_users().wrap_err("Could not get logged in users")? {
        let command = format!("sudo -u {name} DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{id}/bus notify-send -t 5000 \"{text}\"");
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .wrap_err("Could not run notify-send")
            .with_note(|| format!("as user: {id}:{name}"))?;
    }

    Ok(())
}
