use std::process::Command;

pub(crate) fn notify_all_users(text: &str) {
    let users = Command::new("loginctl").output().unwrap().stdout;
    let users = String::from_utf8(users).unwrap();
    let users = users
        .lines()
        .filter(|x| x.starts_with(' '))
        .map(|x| x.split(' ').filter(|x| !x.is_empty()))
        .map(|mut x| (x.nth(1).unwrap(), x.next().unwrap()));

    for (uid, username) in users {
        notify(username, uid, text);
    }
}

fn notify(username: &str, uid: &str, text: &str) {
    let command = format!("sudo -u {username} DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus notify-send -t 5000 \"{text}\"");
    Command::new("sh").arg("-c").arg(command).output().unwrap();
}
