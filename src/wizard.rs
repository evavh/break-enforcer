use std::collections::HashSet;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use color_eyre::eyre::Context;
use color_eyre::Result;
use dialoguer::{Confirm, MultiSelect};

use crate::config;
use crate::watch::{BlockableInput, OnlineDevices};

// todo deal with devices with multiple names
pub fn run(devices: &OnlineDevices, custom_config_path: Option<PathBuf>) -> Result<()> {
    let config: HashSet<_> = config::read(custom_config_path.clone())
        .wrap_err("Could not read custom config")?
        .into_iter()
        .collect();

    let mut inputs = devices.list_inputs();
    let mut options: Vec<_> = inputs
        .iter_mut()
        .map(|BlockableInput { names, id }| (names, config.contains(&id)))
        .map(|(names, checked)| {
            names.dedup();
            (
                names
                    .iter()
                    .map(String::as_str)
                    .intersperse("\n\t& ")
                    .collect::<String>(),
                checked,
            )
        })
        .collect();

    loop {
        let Some(selection) = MultiSelect::new()
            .with_prompt("Use up and down arrow keys and space to select. Enter to continue")
            .items_checked(&options)
            .interact_opt()
            .unwrap()
        else {
            println!("No devices selected");
            return Ok(());
        };

        {
            println!("Locking devices, do not press any key!");
            // do not lock while user is still holding down
            // enter from the multiselect
            thread::sleep(Duration::from_secs(2));
            let mut locked = Vec::new();
            for option in &mut options {
                option.1 = false;
            }
            for item in &selection {
                options[*item].1 = true;
                let id = inputs[*item].id;
                locked.push(devices.lock(id)?);
            }

            println!("Try to use them, they should be blocked");
            thread::sleep(Duration::from_secs(8));
            println!("\n\nUnlocking, Stop typing!");
            for lock in locked {
                lock.unlock()?
            }
        }
        thread::sleep(Duration::from_secs(2));

        let Some(ready) = Confirm::new()
            .with_prompt("Are you happy with the blocked devices?")
            .interact_opt()
            .unwrap()
        else {
            println!("Cancelling");
            return Ok(());
        };

        if ready {
            let selected: Vec<_> = inputs
                .iter()
                .enumerate()
                .filter(|(i, _)| selection.contains(i))
                .map(|(_, dev)| dev.id)
                .collect();
            config::write(&selected, custom_config_path).unwrap();
            return Ok(());
        }
    }
}
