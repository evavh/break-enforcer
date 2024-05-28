use zbus::{proxy, Connection};

mod niet_pub {
    use super::*;

    #[proxy(
        interface = "org.break_enforcer.Status1",
        default_service = "org.break_enforcer.Status",
        default_path = "/org/break_enforcer/Status"
    )]
    trait Status {
        #[zbus(property)]
        fn get_status(&self) -> zbus::Result<String>;
    }
}

pub async fn is_active() -> bool {
    let connection = Connection::session().await.unwrap();
    let proxy = niet_pub::StatusProxy::new(&connection).await.unwrap();
    let status = proxy.get_status().await.unwrap();

    status == "hoi"
}
