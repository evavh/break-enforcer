use std::collections::HashSet;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use color_eyre::eyre::Context;
use color_eyre::Result;
use dialoguer::{Confirm, MultiSelect};

use crate::config;
use crate::lock::Device;
use crate::watch::Devices;

// todo deal with devices with multiple names
pub fn list(devices: &Devices, custom_config_path: Option<PathBuf>) -> Result<()> {
    let config: HashSet<_> = config::read(custom_config_path.clone())
        .wrap_err("Could not read custom config")?
        .into_iter()
        .collect();

    let devices = devices.list()?;
    let mut options: Vec<_> = devices
        .iter()
        .map(|dev @ Device { name, .. }| (name, config.contains(dev)))
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
            for item in &selection {
                options[*item].1 = true;
                locked.push(devices[*item].clone().lock()?);
            }

            println!("Try to use them, they should be blocked");
            thread::sleep(Duration::from_secs(8));
            println!("\n\nUnlocking, Stop typing!");
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
            let selected: Vec<_> = devices
                .iter()
                .enumerate()
                .filter(|(i, _)| selection.contains(i))
                .map(|(_, dev)| dev)
                .cloned()
                .collect();
            config::write(&selected, custom_config_path).unwrap();
            return Ok(());
        }
    }
}
