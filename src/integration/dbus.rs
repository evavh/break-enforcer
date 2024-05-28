use std::time::Duration;

use color_eyre::Result;

use zbus::conn::Builder;
use zbus::interface;

pub(crate) struct Status {
    msg: String,
}

#[interface(name = "org.break_enforcer.Status1")]
impl Status {
    #[zbus(property)]
    async fn get_status(&self) -> String {
        format!("{}", self.msg)
    }
}

pub(super) fn maintain_blocking() -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    rt.block_on(maintain())
}

async fn maintain() -> Result<()> {
    let status = Status {
        msg: "break in 5m".to_string(),
    };
    let connection = Builder::system()?
        .name("org.break_enforcer.Status")?
        .serve_at("/org/break_enforcer/Status", status)?
        .build()
        .await?;

    for i in 0..5000 {
        let iface_ref = connection
            .object_server()
            .interface::<_, Status>("/org/break_enforcer/Status")
            .await?;
        let mut iface = iface_ref.get_mut().await;
        iface.msg = format!("break in {i}m");
        iface.get_status_changed(iface_ref.signal_context()).await?;

        tokio::time::sleep(Duration::from_secs(10)).await;
    }

    Ok(())
}
