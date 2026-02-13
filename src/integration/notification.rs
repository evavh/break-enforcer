use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use color_eyre::eyre::{eyre, Context};
use color_eyre::{Result, Section};

use crate::integration;

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
    let users = String::from_utf8(users)
        .wrap_err("loginctl could not be parsed as utf8")?;
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

pub(crate) fn beep_all_users() -> Result<()> {
    fn beep(name: String, id: String) -> Result<()> {
        let sound1 = include_bytes!(
            "../../assets/new-notification-on-your-device-by-UNIVERSFIELD.wav"
        );

        let uid: u32 = id.parse().wrap_err("Failed to parse user id")?;
        let mut aplay = Command::new("aplay")
            .env("XDG_RUNTIME_DIR", format!("/run/user/{id}"))
            .uid(uid)
            .stdin(Stdio::piped())
            .spawn()
            .wrap_err("Could not spawn shell")
            .with_note(|| format!("as user: {id}:{name}"))?;

        let stdin = aplay.stdin.as_mut().expect("is set to piped");
        stdin
            .write_all(sound1)
            .wrap_err("Could not pipe to aplay")?;
        aplay.wait().wrap_err("Could not wait for command to end")?;
        Ok(())
    }

    for User { id, name } in
        all_users().wrap_err("Could not get logged in users")?
    {
        let _ = std::thread::spawn(|| {
            if let Err(report) = beep(name, id).wrap_err("beep failed") {
                eprintln!("{report:?}");
            }
        });
    }

    Ok(())
}

pub(crate) fn command_available(
    cmd: &str,
    expected_output: &str,
    packages_help: &'static str,
) -> color_eyre::Result<()> {
    match Command::new(cmd).arg("--version").output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains(expected_output) {
                Ok(())
            } else {
                Err(eyre!("{cmd} is in path but gave strange output")
                    .with_note(|| format!("{cmd} output: {stdout}")))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(eyre!("could not find {cmd} in path")).suggestion(packages_help)
        }
        Err(e) => {
            Err(e).wrap_err("Could not investigate whether aplay is installed")
        }
    }
}

pub(crate) fn beep_available() -> color_eyre::Result<()> {
    command_available(
        "aplay",
        "aplay: version",
        "on most systems aplay is provided by alsa-utils",
    )
}

pub(crate) fn notify(text: &str) -> Result<()> {
    for User { id, name } in
        all_users().wrap_err("Could not get logged in users")?
    {
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

pub(crate) fn notify_available() -> color_eyre::Result<()> {
    command_available(
        "notify-send",
        "notify-send ",
        "provided by the package libnotify-bin or libnotify",
    )
}

pub(crate) fn run_command(c: &integration::Command) -> color_eyre::Result<()> {
    let output = Command::new(&c.program)
        .args(&c.args)
        .output()
        .wrap_err("Could not spawn user provided command")?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Err(eyre!("User provided command failed")
            .with_note(|| format!("stderr: {stderr}"))
            .with_note(|| format!("stdout: {stdout}")))
    }
}
